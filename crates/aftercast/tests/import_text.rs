mod common;
use aftercast::{
    Error, Host, ImportEncoding, ImportInvalidBytes, ImportRange, ImportTextOptions, PluginConfig,
};
use camino::Utf8Path;
use common::{Deployment, deployment};

async fn parser(host: &Host) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("fixture");
    host.initialize_plugin(plugin).await.expect("initialize");
}
fn encode(encoding: ImportEncoding, text: &str) -> Vec<u8> {
    match encoding {
        ImportEncoding::Utf8 => text.as_bytes().to_vec(),
        ImportEncoding::Utf16Le => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        ImportEncoding::Utf16Be => text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        ImportEncoding::Utf32Le => text
            .chars()
            .flat_map(|c| u32::from(c).to_le_bytes())
            .collect(),
        ImportEncoding::Utf32Be => text
            .chars()
            .flat_map(|c| u32::from(c).to_be_bytes())
            .collect(),
        ImportEncoding::ShiftJis => legacy(text, "日本語", &[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea]),
        ImportEncoding::WindowsKorean => legacy(text, "한글", &[0xc7, 0xd1, 0xb1, 0xdb]),
        ImportEncoding::Windows1252 => legacy(text, "café", b"caf\xe9"),
        ImportEncoding::Ascii => legacy(text, "ascii", b"ascii"),
    }
}
fn legacy(text: &str, word: &str, bytes: &[u8]) -> Vec<u8> {
    // Fixed independently encoded fixture words; this helper is not a general charset converter.
    let mut output = Vec::new();
    for (index, part) in text.split(word).enumerate() {
        assert!(part.is_ascii(), "fixture contains an unencoded character");
        if index != 0 {
            output.extend_from_slice(bytes);
        }
        output.extend_from_slice(part.as_bytes());
    }
    output
}

async fn source(host: &Host, index: u32) -> Vec<String> {
    let graph = host.encounters(index, 1).await.expect("graph");
    let id = graph.items.first().expect("encounter").id;
    host.encounter_logs(id, 0, 16)
        .await
        .expect("source logs")
        .expect("scope")
        .page
        .items
        .into_iter()
        .map(|item| item.text)
        .collect()
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn explicit_text_options_round_trip_whole_and_inclusive_range_imports(
    deployment: Deployment,
) {
    let host = Host::start(deployment.config()).await.expect("host");
    parser(&host).await;
    let input = deployment.directory().join("encoding 한글.log");
    let epoch = 621_355_968_000_000_000;
    let range = ImportRange::new(epoch + 10_000_000, epoch + 20_000_000).expect("range");
    let cases = [
        ImportEncoding::Utf8,
        ImportEncoding::Utf16Le,
        ImportEncoding::Utf16Be,
        ImportEncoding::Utf32Le,
        ImportEncoding::Utf32Be,
        ImportEncoding::ShiftJis,
        ImportEncoding::WindowsKorean,
        ImportEncoding::Windows1252,
        ImportEncoding::Ascii,
    ];
    for (index, encoding) in cases.into_iter().enumerate() {
        let word = match encoding {
            ImportEncoding::ShiftJis => "日本語",
            ImportEncoding::WindowsKorean => "한글",
            ImportEncoding::Windows1252 => "café",
            ImportEncoding::Ascii => "ascii",
            _ => "한글日本語🙂",
        };
        let text =
            format!("0|100|{word}\r\n1|200|{word}\r1|300|{word}\n2|400|{word}\n3|999|{word}");
        std::fs::write(&input, encode(encoding, &text)).expect("source");
        let mut selected = ImportTextOptions {
            encoding,
            ..Default::default()
        };
        let job = host
            .start_import_with_options(&input, selected)
            .await
            .expect("admission");
        selected.encoding = ImportEncoding::Ascii;
        let report = job.wait().await.expect("captured options");
        assert_eq!((report.records_dispatched, report.damage), (5, 1999));
        assert_eq!(
            source(&host, index as u32 * 2)
                .await
                .first()
                .expect("first source record")
                .as_str(),
            format!("0|100|{word}")
        );
        let report = host
            .start_import_range_with_options(
                &input,
                range,
                ImportTextOptions {
                    encoding,
                    ..selected
                },
            )
            .await
            .expect("range admission")
            .wait()
            .await
            .expect("range");
        assert_eq!((report.records_dispatched, report.damage), (3, 900));
        assert_eq!(
            source(&host, index as u32 * 2 + 1).await,
            [
                format!("1|200|{word}"),
                format!("1|300|{word}"),
                format!("2|400|{word}")
            ]
        );
    }
    failures_and_bom(&host, &input).await;
    cancellation(&host, &input).await;
    let closed = host.handle();
    host.shutdown().await.expect("shutdown");
    let fresh = Host::start(deployment.config()).await.expect("restart");
    parser(&fresh).await;
    std::fs::write(&input, "0|1|fresh\n").expect("fresh source");
    closed
        .start_import_with_options(&input, ImportTextOptions::default())
        .await
        .err()
        .expect("closed session cannot admit text requests");
    assert_eq!(
        fresh
            .start_import_with_options(&input, ImportTextOptions::default())
            .await
            .expect("fresh admission")
            .wait()
            .await
            .expect("fresh import")
            .damage,
        1
    );
    fresh.shutdown().await.expect("stop");
}
async fn failures_and_bom(host: &Host, input: &Utf8Path) {
    let mut encoded = vec![0xff, 0xfe];
    encoded.extend(encode(ImportEncoding::Utf16Le, "0|10|한글\n"));
    std::fs::write(input, encoded).expect("BOM source");
    assert_eq!(
        host.start_import_with_options(input, ImportTextOptions::default())
            .await
            .expect("BOM admission")
            .wait()
            .await
            .expect("BOM override")
            .damage,
        10
    );
    let disabled = ImportTextOptions {
        detect_bom: false,
        ..Default::default()
    };
    host.start_import_with_options(input, disabled)
        .await
        .expect("disabled BOM admission")
        .wait()
        .await
        .expect_err("strict UTF-8 must not guess UTF-16");
    let before = host.encounters(0, 256).await.expect("before partial").total;
    std::fs::write(input, b"0|100|valid\n1|200|\xff\n").expect("malformed suffix");
    let error = host
        .start_import_with_options(input, ImportTextOptions::default())
        .await
        .expect("strict admission")
        .wait()
        .await
        .expect_err("strict failure");
    assert!(matches!(error, Error::Managed { command: 8, .. }));
    assert_eq!(
        host.encounters(before, 1)
            .await
            .expect("partial")
            .items
            .first()
            .expect("partial encounter")
            .damage,
        100
    );
    let replace = ImportTextOptions {
        invalid_bytes: ImportInvalidBytes::Replace,
        ..Default::default()
    };
    assert_eq!(
        host.start_import_with_options(input, replace)
            .await
            .expect("replace admission")
            .wait()
            .await
            .expect("replace")
            .damage,
        300
    );
    assert!(
        source(host, before + 1)
            .await
            .get(1)
            .expect("replacement record")
            .contains('\u{fffd}')
    );
    host.start_import(input)
        .await
        .expect("default admission")
        .wait()
        .await
        .expect_err("replacement was per request; default remains strict");
}
async fn cancellation(host: &Host, input: &Utf8Path) {
    let bytes = encode(
        ImportEncoding::Utf16Be,
        &"1|1|한글日本語🙂\n".repeat(100_000),
    );
    std::fs::write(input, &bytes).expect("large source");
    let before = host.encounters(0, 256).await.expect("before cancel").total;
    let mut job = host
        .start_import_with_options(
            input,
            ImportTextOptions {
                encoding: ImportEncoding::Utf16Be,
                ..Default::default()
            },
        )
        .await
        .expect("cancel admission");
    tokio::time::timeout(std::time::Duration::from_secs(10),async {
        loop {
            let progress = job.progress();
            assert!(progress.bytes_read <= progress.total_bytes && progress.records_dispatched <= progress.lines_read);
            if progress.records_dispatched > 0 { break; }
            tokio::select! { result = &mut job => panic!("completed before cancellation: {result:?}"), () = tokio::time::sleep(std::time::Duration::from_millis(1)) => {} }
        }
    }).await.expect("progress");
    job.cancel();
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    let partial = host.encounters(before, 1).await.expect("partial cancel");
    let damage = partial.items.first().expect("cancelled encounter").damage;
    assert!(damage > 0 && damage < 100_000);
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn omitted_options_inherit_plugin_encoding_without_explicit_request_leak(
    deployment: Deployment,
) {
    let host = Host::start(deployment.config()).await.expect("host");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.KoreanEncodingPlugin")
                .build(),
        )
        .await
        .expect("encoding fixture");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = deployment.directory().join("inherit.log");
    std::fs::write(&input, encode(ImportEncoding::WindowsKorean, "0|1|한글\n"))
        .expect("Korean source");
    assert_eq!(
        host.import_log(&input)
            .await
            .expect("inherit plugin selection")
            .damage,
        1
    );
    assert_eq!(source(&host, 0).await, ["0|1|한글"]);
    std::fs::write(&input, "0|2|日本語🙂\n").expect("UTF8 source");
    assert_eq!(
        host.start_import_with_options(&input, ImportTextOptions::default())
            .await
            .expect("explicit UTF8")
            .wait()
            .await
            .expect("override")
            .damage,
        2
    );
    assert_eq!(source(&host, 1).await, ["0|2|日本語🙂"]);
    std::fs::write(&input, encode(ImportEncoding::WindowsKorean, "0|3|한글\n"))
        .expect("Korean source again");
    assert_eq!(
        host.import_log(&input)
            .await
            .expect("plugin selection unchanged")
            .damage,
        3
    );
    assert_eq!(source(&host, 2).await, ["0|3|한글"]);
    host.deinitialize_plugin(plugin)
        .await
        .expect("restore prior encoding");
    parser(&host).await;
    host.start_import(&input)
        .await
        .expect("restored admission")
        .wait()
        .await
        .expect_err("restored default is strict UTF8");
    host.shutdown().await.expect("stop");
}
