//! Scan complet contre un faux serveur Blizzard (wiremock).

use std::sync::Mutex;

use serde_json::json;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wowscan_core::cache::Cache;
use wowscan_core::gamedata::GameData;
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
    let result = scan(&client, &cache, &[100, 200, 300, 100], None, 4, |e| events.lock().unwrap().push(e))
        .await
        .unwrap();

    assert!(result.failures.is_empty());
    assert_eq!(result.realms.len(), 2);
    assert_eq!(result.items.len(), 3, "doublons retirés");

    let sword = &result.items[0];
    assert_eq!(sword.item_id, 100);
    assert_eq!(sword.variants.len(), 1, "without game tables, one version per item");
    let sword = &sword.variants[0];
    assert_eq!(sword.commodity, None);
    assert_eq!(sword.realms.iter().map(|r| r.connected_realm_id).collect::<Vec<_>>(), vec![2, 1]);
    assert_eq!(sword.realms[1].stats.min, Money(4_000_000));
    assert_eq!(sword.realms[1].stats.auctions, 2);

    let herb = result.items[1].variants[0].commodity.unwrap();
    assert_eq!((herb.min, herb.quantity), (Money(100), 60));
    assert!(result.items[2].variants.is_empty());

    let events = events.into_inner().unwrap();
    assert!(matches!(events[0], ScanEvent::Started { total: 3 }));
    assert_eq!(events.iter().filter(|e| matches!(e, ScanEvent::SourceDone { downloaded: true, .. })).count(), 3);

    // Rescanner aussitôt : tout vient du cache, sans requête d'enchères.
    let before = server.received_requests().await.unwrap().len();
    let again = scan(&client, &cache, &[100], None, 4, |_| {}).await.unwrap();
    assert_eq!(again.items[0].variants[0].realms.len(), 2);
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

/// Tables du jeu minimales (extraits réels) pour la ceinture 124182, servies comme wago.tools.
async fn mount_game_tables(server: &MockServer) {
    let tables = [
        ("ItemBonus", "ID,Value_0,Value_1,Value_2,Value_3,ParentItemBonusListID,Type,OrderIndex\n\
            1,4,0,0,0,9079,3,0\n2,2015,0,0,0,9080,4,0\n3,1,0,0,0,9080,7,0\n\
            4,13145,0,0,0,9081,4,0\n5,3,0,0,0,9081,7,0\n"),
        ("ItemModifiedAppearance", "ID,ItemID,ItemAppearanceModifierID,ItemAppearanceID,OrderIndex,TransmogSourceTypeEnum,Flags\n\
            10,124182,0,25447,0,0,0\n11,124182,1,25419,0,0,0\n12,124182,3,25526,0,0,0\n"),
        ("TransmogSetItem", "ID,TransmogSetID,ItemModifiedAppearanceID,Flags\n1,500,12,0\n"),
        ("ItemNameDescription", "ID,Description_lang,Color\n2015,Heroic,65280\n13145,Mythic,-16711936\n"),
        ("TransmogSet", "Name_lang,ID,ClassMask,TrackingQuestID,Flags,TransmogSetGroupID,ItemNameDescriptionID,ParentTransmogSetID,CompleteWorldStateID,ExpansionID,PatchIntroduced,UiOrder,ConditionID\n\
            \"Vestments of Unhinged Malice\",500,0,0,0,0,13145,0,0,5,0,0,0\n"),
        ("JournalEncounterItem", "ID,JournalEncounterID,ItemID,FactionMask,Flags,DifficultyMask,DisplaySeasonID,WorldStateExpressionID\n\
            1,1438,124182,-1,0,0,0,0\n"),
        ("JournalEncounter", "Name_lang,Description_lang,Map_0,Map_1,ID,JournalInstanceID,DungeonEncounterID,OrderIndex\n\
            Archimonde,,0,0,1438,669,1799,13\n"),
        ("JournalInstance", "ID,Name_lang,Description_lang,MapID\n669,Hellfire Citadel,,1448\n"),
        ("Item", "ID,ClassID,SubclassID,Material,InventoryType,SheatheType\n124182,4,3,5,6,0\n"),
        ("Map", "ID,Directory,InstanceType,ExpansionID\n1448,HellfireRaid,2,5\n"),
        ("Mount", "Name_lang,SourceText_lang,Description_lang,ID,MountTypeID,Flags,SourceTypeEnum,SourceSpellID\n\
            Felblaze Infernal,\"|cFFFFD200Drop: |rArchimonde|n\",,791,0,0,0,232405\n\
            Hidden,,,5,0,0,-1,1\n"),
        ("ItemEffect", "ID,LegacySlotIndex,TriggerType,Charges,CoolDownMSec,CategoryCoolDownMSec,SpellCategoryID,SpellID\n7,0,6,0,0,0,0,232405\n"),
        ("ItemXItemEffect", "ID,ItemEffectID,ItemID\n1,7,124182\n"),
        ("JournalTier", "ID,Name_lang,Expansion\n7,Warlords,600\n"),
        ("JournalTierXInstance", "ID,JournalTierID,JournalInstanceID\n1,7,669\n"),
        ("SpellMisc", "ID,DifficultyID,SpellIconFileDataID,SpellID\n1,0,0,1\n2,0,1394953,232405\n3,14,99,232405\n"),
        ("ManifestInterfaceData", "ID,FilePath,FileName\n1394953,Interface\\ICONS\\,INV_Test_Mount.blp\n"),
    ];
    for (table, csv) in tables {
        Mock::given(method("GET"))
            .and(path(format!("/db2/{table}/csv")))
            .respond_with(ResponseTemplate::new(200).set_body_string(csv))
            .mount(server)
            .await;
    }
}

#[tokio::test]
async fn splits_items_by_version_with_game_tables() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"access_token": "tok", "expires_in": 86399})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/data/wow/connected-realm/index"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"connected_realms": [
            {"href": format!("{}/data/wow/connected-realm/1?namespace=dynamic-eu", server.uri())},
        ]})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/data/wow/connected-realm/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1, "realms": [{"id": 1, "name": "Elune", "slug": "elune"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/data/wow/connected-realm/1/auctions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"auctions": [
            {"id": 1, "item": {"id": 124182, "bonus_lists": [9081]}, "buyout": 25_000_000, "quantity": 1},
            {"id": 2, "item": {"id": 124182, "bonus_lists": [9079]}, "buyout": 149_990_000, "quantity": 1},
            {"id": 3, "item": {"id": 124182, "bonus_lists": [9080]}, "buyout": 199_990_000, "quantity": 1},
            {"id": 4, "item": {"id": 124182, "bonus_lists": [9081]}, "buyout": 50_000_000, "quantity": 1},
        ]})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/data/wow/auctions/commodities"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"auctions": []})))
        .mount(&server)
        .await;
    mount_game_tables(&server).await;

    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let data = GameData::load_from(&cache, &format!("{}/db2", server.uri())).await.unwrap();
    assert_eq!(data.set(500).unwrap().pieces[0].appearance_id, 25526);
    assert_eq!(data.set(500).unwrap().category, wowscan_core::gamedata::SetCategory::Raid);
    let raid = data.set(500).unwrap().raid.as_ref().unwrap();
    assert_eq!((raid.instance_id, raid.difficulty), (669, wowscan_core::gamedata::SetDifficulty::Mythic));
    assert_eq!(raid.name.get("en_GB"), "Hellfire Citadel");
    assert_eq!(data.label(13145).unwrap().get("fr_FR"), "Mythic", "same table served for both locales");
    assert_eq!(data.drops(124182)[0].instance.get("en_GB"), "Hellfire Citadel");
    assert_eq!(data.slot(124182), Some(6));
    let mounts = data.mounts();
    assert_eq!(mounts.len(), 1, "hidden mounts are left out");
    assert_eq!((mounts[0].id, mounts[0].spell_id, mounts[0].item_id), (791, 232405, Some(124182)));
    assert_eq!(mounts[0].icon, Some(1394953));
    assert_eq!(data.icon_name(1394953), Some("INV_Test_Mount"));
    assert_eq!((mounts[0].category, mounts[0].expansion), (wowscan_core::mounts::MountCategory::Raid, 5));
    assert_eq!(mounts[0].source.get("en_GB"), "Drop: Archimonde");

    let result = scan(&client(&server, "secret"), &cache, &[124182], Some(&data), 2, |_| {}).await.unwrap();
    let variants: Vec<(Option<u32>, Money, Vec<u32>)> = result.items[0]
        .variants
        .iter()
        .map(|v| (v.appearance_id, v.realms[0].stats.min, v.labels.clone()))
        .collect();
    assert_eq!(
        variants,
        vec![
            (Some(25526), Money(25_000_000), vec![13145]),
            (Some(25447), Money(149_990_000), vec![]),
            (Some(25419), Money(199_990_000), vec![2015]),
        ],
        "Mythic, Normal and Heroic are kept apart"
    );
    assert_eq!(result.items[0].variants[0].realms[0].stats.auctions, 2);
}

#[tokio::test]
async fn keeps_items_fetched_before_a_failure() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path("/data/wow/item/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1, "name": "One", "quality": {"type": "EPIC"},
            "item_class": {"name": "Armor"}, "item_subclass": {"name": "Plate"}, "inventory_type": {"name": "Head"}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/data/wow/item/2"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let client = client(&server, "secret");
    let found = wowscan_core::items::item_infos(&client, &cache, &[1, 2]).await.unwrap();
    assert_eq!(found.keys().copied().collect::<Vec<_>>(), vec![1]);
    let again = wowscan_core::items::item_infos(&client, &cache, &[1]).await.unwrap();
    assert_eq!(again[&1].quality, "EPIC");
    assert!(wowscan_core::items::item_infos(&client, &cache, &[2]).await.is_err(), "nothing found: the error is reported");
}
