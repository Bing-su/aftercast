mod common;

use aftercast::{Host, HostConfig, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn idle_options_apply_to_each_worker_session(deployment: Deployment) {
    let input = deployment.directory().join("idle.log");
    std::fs::write(
        &input,
        "0|100|Strike|Source|Target\n10|100|Strike|Source|Target\n",
    )
    .expect("synthetic input");
    for (enabled, seconds, expected) in [(true, 6, 2), (false, 6, 1), (true, 30, 1)] {
        let config = if enabled && seconds == 6 {
            deployment.config()
        } else {
            HostConfig::builder()
                .application_executable(common::application())
                .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
                .bridge_directory(&deployment.bridge)
                .data_directory(&deployment.state)
                .idle_end_enabled(enabled)
                .idle_end_seconds(seconds)
                .build()
        };
        let host = Host::start(config).await.expect("host");
        let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        );
        let plugin = host
            .create_plugin(
                PluginConfig::builder()
                    .path(path)
                    .type_name("Aftercast.PluginFixture.VcrImportPlugin")
                    .build(),
            )
            .await
            .expect("plugin");
        host.initialize_plugin(plugin).await.expect("initialize");
        host.import_log(&input).await.expect("import");
        assert_eq!(
            host.encounters(0, 16).await.expect("encounters").total,
            expected
        );
        host.shutdown().await.expect("shutdown");
    }
}
