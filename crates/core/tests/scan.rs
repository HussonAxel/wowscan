//! Scan complet contre un faux serveur Blizzard (wiremock).

use std::sync::Mutex;

use serde_json::json;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wowscan_core::cache::Cache;
use wowscan_core::scan::{ScanEvent, scan};
use wowscan_core::{BlizzardClient, Credentials, Error, Money, Region};

const LAST_MODIFIED: &str = "Wed, 28 Oct 2026 20:00:00 GMT";

async fn server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"access_token": "tok", "expires_in": 86399})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/data/wow/connected-realm/index"))
        .and(header("Battlenet-Namespace", "dynamic-eu"))
        .and(header("Authorization", "Bearer tok"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"connected_realms": [
            {"href": format!("{}/data/wow/connected-realm/1?namespace=dynamic-eu", server.uri())},
            {"href": format!("{}/data/wow/connected-realm/2?namespace=dynamic-eu", server.uri())},
        ]})))
        .mount(&server)
        .await;
    for (id, name) in [(1, "Hyjal"), (2, "Kirin Tor")] {
        Mock::given(method("GET"))
            .and(path(format!("/data/wow/connected-realm/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": id, "realms": [{"id": id, "name": {"fr_FR": name, "en_GB": name}, "slug": name.to_lowercase()}]
            })))
            .mount(&server)
            .await;
    }
    // Second scan : la requête conditionnelle reçoit 304 (monté en premier, donc prioritaire).
    Mock::given(method("GET"))
        .and(header_exists("If-Modified-Since"))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;
    let auctions = |body| ResponseTemplate::new(200).insert_header("Last-Modified", LAST_MODIFIED).set_body_json(body);
    Mock::given(method("GET"))
        .and(path("/data/wow/connected-realm/1/auctions"))
        .respond_with(auctions(json!({"auctions": [
            {"id": 1, "item": {"id": 100}, "buyout": 5_000_000, "quantity": 1},
            {"id": 2, "item": {"id": 100}, "buyout": 4_000_000, "quantity": 1},
            {"id": 3, "item": {"id": 999}, "buyout": 1, "quantity": 1},
        ]})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/data/wow/connected-realm/2/auctions"))
        .respond_with(auctions(json!({"auctions": [{"id": 4, "item": {"id": 100}, "buyout": 3_000_000, "quantity": 1}]})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/data/wow/auctions/commodities"))
        .respond_with(auctions(json!({"auctions": [
            {"id": 5, "item": {"id": 200}, "unit_price": 120, "quantity": 50},
            {"id": 6, "item": {"id": 200}, "unit_price": 100, "quantity": 10},
        ]})))
        .mount(&server)
        .await;
    server
}

fn client(server: &MockServer, secret: &str) -> BlizzardClient {
    let credentials = Credentials { client_id: "id".into(), client_secret: secret.into() };
    BlizzardClient::with_urls(credentials, Region::Eu, &server.uri(), &format!("{}/token", server.uri())).unwrap()
}

#[tokio::test]
async fn scans_realms_and_commodities_then_uses_cache() {
    let server = server().await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let client = client(&server, "secret");

    let events = Mutex::new(Vec::new());
    let result = scan(&client, &cache, &[100, 200, 300, 100], 4, |e| events.lock().unwrap().push(e))
        .await
        .unwrap();

    assert!(result.failures.is_empty());
    assert_eq!(result.realms.len(), 2);
    assert_eq!(result.items.len(), 3, "doublons retirés");

    let sword = &result.items[0];
    assert_eq!(sword.item_id, 100);
    assert_eq!(sword.commodity, None);
    assert_eq!(sword.realms.iter().map(|r| r.connected_realm_id).collect::<Vec<_>>(), vec![2, 1]);
    assert_eq!(sword.realms[1].stats.min, Money(4_000_000));
    assert_eq!(sword.realms[1].stats.auctions, 2);

    let herb = result.items[1].commodity.unwrap();
    assert_eq!((herb.min, herb.quantity), (Money(100), 60));
    assert!(result.items[2].realms.is_empty() && result.items[2].commodity.is_none());

    let events = events.into_inner().unwrap();
    assert!(matches!(events[0], ScanEvent::Started { total: 3 }));
    assert_eq!(events.iter().filter(|e| matches!(e, ScanEvent::SourceDone { downloaded: true, .. })).count(), 3);

    // Rescanner aussitôt : tout vient du cache, sans requête d'enchères.
    let before = server.received_requests().await.unwrap().len();
    let again = scan(&client, &cache, &[100], 4, |_| {}).await.unwrap();
    assert_eq!(again.items[0].realms.len(), 2);
    assert_eq!(server.received_requests().await.unwrap().len(), before);
}

#[tokio::test]
async fn rejects_bad_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let err = client(&server, "wrong").check_credentials().await.unwrap_err();
    assert!(matches!(err, Error::InvalidCredentials));
}
