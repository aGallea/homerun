use anyhow::Result;
use homerun::client::DaemonClient;

fn accepts_fallible_socket_constructor(_: Result<DaemonClient>) {}

#[test]
fn default_socket_constructor_remains_fallible() {
    accepts_fallible_socket_constructor(DaemonClient::default_socket());
}
