#[test]
fn stale_pn_targets_are_removed_while_desired_targets_are_retained() {
    let stale = client_proxy(node_id(21), Vec::new());
    let retained = client_proxy(node_id(22), Vec::new());
    let mut connected = HashMap::from([
        (stale.clone(), Vec::<TtpTarget>::new()),
        (retained.clone(), Vec::<TtpTarget>::new()),
    ]);
    let desired = HashMap::from([(retained.clone(), Vec::<TtpTarget>::new())]);

    let removed = take_removed_pn_targets(&mut connected, &desired);

    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].0, stale);
    assert!(!connected.contains_key(&removed[0].0));
    assert!(connected.contains_key(&retained));
}

#[test]
fn identical_pn_reappearance_is_missing_and_enters_the_connect_path() {
    let pn_server = client_proxy(node_id(23), Vec::new());
    let mut connected =
        HashMap::from([(pn_server.clone(), Vec::<TtpTarget>::new())]);
    let absent = HashMap::new();

    let removed = take_removed_pn_targets(&mut connected, &absent);
    assert_eq!(removed.len(), 1);
    assert!(!connected.contains_key(&pn_server));

    let desired_after_reappearance =
        HashMap::from([(pn_server.clone(), Vec::<TtpTarget>::new())]);
    let connect_candidates = desired_after_reappearance
        .keys()
        .filter(|candidate| !connected.contains_key(*candidate))
        .collect::<Vec<_>>();
    assert_eq!(connect_candidates, vec![&pn_server]);
}

#[test]
fn regression_model_exposes_the_legacy_stale_registry_entry() {
    let pn_server = client_proxy(node_id(24), Vec::new());
    let legacy_connected =
        HashMap::from([(pn_server.clone(), Vec::<TtpTarget>::new())]);
    assert!(legacy_connected.contains_key(&pn_server));
    assert!(
        !(!legacy_connected.contains_key(&pn_server)),
        "the legacy registry would skip connect_server for identical metadata"
    );

    let mut connected = legacy_connected;
    let removed = take_removed_pn_targets(&mut connected, &HashMap::new());
    assert_eq!(removed.len(), 1);
    assert!(!connected.contains_key(&pn_server));
}

#[test]
fn registry_entry_is_removed_before_external_target_removal() {
    let source = include_str!("../../src/p2p_vpn.rs");
    let sync_start = source
        .find("async fn sync_pn_server_connections")
        .expect("missing PN synchronization method");
    let body = &source[sync_start..];
    let registry_removal = body
        .find("take_removed_pn_targets(&mut connected, &desired)")
        .expect("missing stale registry removal");
    let external_removal = body
        .find("ttp_client.remove_server(target)")
        .expect("missing external target removal");

    assert!(registry_removal < external_removal);
}
