mod common;

use aftercast::{AudioSettings, Error, Host, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};
use std::time::Duration;

async fn plugin(host: &Host, name: &str) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name(format!("Aftercast.PluginFixture.{name}"))
                .build(),
        )
        .await
        .expect("plugin");
    host.initialize_plugin(id).await.expect("initialize");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn audio_is_created_played_and_disposed_in_the_worker(deployment: Deployment) {
    assert!(deployment.state.starts_with(deployment.directory()));
    let mut host = Host::start(deployment.config()).await.expect("host");
    assert_ne!(host.process_id(), std::process::id());
    host.set_tts(true).await.expect("TTS");
    host.set_sound_player(true).await.expect("sound");
    host.set_audio_settings(&AudioSettings {
        tts_volume: 37,
        sound_volume: 61,
        timer_expire: "tts expired".into(),
        ..AudioSettings::default()
    })
    .await
    .expect("settings");
    plugin(&host, "SpeechPlugin").await;
    plugin(&host, "SoundPlugin").await;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let tts = std::fs::read_to_string(deployment.state.join("tts.txt")).expect("TTS records");
        let sound =
            std::fs::read_to_string(deployment.state.join("sound.txt")).expect("sound records");
        if tts.contains("37| expired") && sound.contains("61|beep") {
            assert!(tts.contains("37|한글 <plain> & text"));
            assert!(tts.contains("37| Caster"));
            assert!(sound.contains("fixture.wav"));
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "audio/timer delivery stalled"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    for _ in 0..2 {
        plugin(&host, "FailingSpeechPlugin").await;
    }
    for _ in 0..2 {
        let error = tokio::time::timeout(Duration::from_secs(5), host.next_audio_error())
            .await
            .expect("audio failure deadline")
            .expect("audio error");
        assert!(matches!(error, Error::Speech(_)), "{error:?}");
        assert!(error.to_string().contains("fixture playback failure"));
    }
    plugin(&host, "AudioCleanupPlugin").await;
    std::fs::write(deployment.state.join("audio.probe"), "enabled")
        .expect("enable retirement probe");
    tokio::time::timeout(Duration::from_secs(2), host.clear_tts())
        .await
        .expect("audio retirement must keep pumping owner callbacks")
        .expect("disable TTS");
    host.clear_sound_player().await.expect("disable sound");
    host.shutdown().await.expect("cleanup and reap");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = restarted
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.SpeechPlugin")
                .build(),
        )
        .await
        .expect("plugin");
    assert!(
        restarted.initialize_plugin(id).await.is_err(),
        "audio routes must not survive restart"
    );
    restarted.shutdown().await.expect("restart cleanup");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn shutdown_retires_audio_before_disposing_its_owner_context(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    host.set_tts(true).await.expect("TTS");
    plugin(&host, "AudioCleanupPlugin").await;
    std::fs::write(deployment.state.join("audio.probe"), "enabled").expect("probe");
    tokio::time::timeout(Duration::from_secs(3), host.shutdown())
        .await
        .expect("shutdown must keep pumping audio cleanup")
        .expect("graceful cleanup");
    assert!(
        deployment.state.join("audio.released").exists(),
        "cleanup required an owner callback"
    );
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn speech_rules_are_session_owned_and_persist_without_playback(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let initial = host.speech_rules().await.expect("initial rules");
    assert!(initial.rules.is_empty());
    let rules = vec![aftercast::SpeechRule {
        pattern: "(cat)".into(),
        replacement: "$1dog".into(),
        active: true,
    }];
    let saved = host
        .save_speech_rules(initial.revision, &rules)
        .await
        .expect("save");
    assert_eq!(saved.rules, rules);
    host.set_audio_settings(&AudioSettings {
        sound_volume: 61,
        tts_volume: 37,
        ..AudioSettings::default()
    })
    .await
    .expect("audio settings");
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(assembly)
                .type_name("Aftercast.PluginFixture.SpeechRulesProbePlugin")
                .build(),
        )
        .await
        .expect("fake delegate plugin");
    assert_eq!(
        host.initialize_plugin(id)
            .await
            .expect("in-memory delegate"),
        "CATdog|61|37"
    );
    host.save_speech_rules(initial.revision, &rules)
        .await
        .expect_err("stale");
    let invalid = vec![aftercast::SpeechRule {
        pattern: "[".into(),
        replacement: "x".into(),
        active: true,
    }];
    host.save_speech_rules(saved.revision, &invalid)
        .await
        .expect_err("invalid regex");
    assert_eq!(host.speech_rules().await.expect("unchanged").rules, rules);
    assert!(
        !deployment.state.join("tts.txt").exists(),
        "editing must not start playback"
    );
    host.shutdown().await.expect("shutdown");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    assert_eq!(
        restarted.speech_rules().await.expect("restored").rules,
        rules
    );
    restarted
        .save_speech_rules(saved.revision, &rules)
        .await
        .expect_err("old session");
    restarted.shutdown().await.expect("shutdown");
}
