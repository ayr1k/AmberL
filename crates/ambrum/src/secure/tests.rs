// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::auth::{
    Endpoint,
    tests::{ID, Mock, chain},
};
use serde_json::json;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("ambrum-secure-{}", new_id().unwrap()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn authorized() -> Authorization {
    Authorization {
        identity: Identity {
            profile: Profile {
                id: ID.into(),
                name: "TestPlayer".into(),
            },
            token: Secret::new("minecraft-secret".into()).unwrap(),
            xuid: "123".into(),
            expires: std::time::Instant::now() + std::time::Duration::from_secs(60),
        },
        refresh: Secret::new("refresh-private-secret".into()).unwrap(),
    }
}
#[test]
fn dpapi_roundtrip_reopen_and_logout_keep_no_plaintext_credentials_on_disk() {
    check_available().unwrap();
    let f = Fixture::new();
    {
        let mut store = Store::open(&f.0).unwrap();
        store.save_authorization(authorized()).unwrap();
        assert_eq!(store.profiles()[0].id, ID);
        assert!(Store::open(&f.0).is_err());
    }
    let bytes = fs::read(f.0.join(FILE)).unwrap();
    assert!(!bytes.windows(7).any(|w| w == b"refresh"));
    {
        let mut store = Store::open(&f.0).unwrap();
        assert_eq!(store.profiles().len(), 1);
        store.forget(ID).unwrap();
    }
    assert!(Store::open(&f.0).unwrap().profiles().is_empty());
    assert!(!fs::read_dir(&f.0).unwrap().any(|e| {
        e.unwrap()
            .path()
            .extension()
            .is_some_and(|s| s == "encrypted-part")
    }));
}
#[test]
fn damaged_future_schema_and_linked_accounts_are_preserved() {
    for mode in 0..3 {
        let f = Fixture::new();
        {
            Store::open(&f.0)
                .unwrap()
                .save_authorization(authorized())
                .unwrap();
        }
        let path = f.0.join(FILE);
        if mode == 0 {
            fs::write(&path, b"not-dpapi").unwrap();
        }
        if mode == 1 {
            let data = Data {
                schema: 99,
                application: APPLICATION_ID.into(),
                accounts: vec![],
            };
            fs::write(
                &path,
                &*protect(&serde_json::to_vec(&data).unwrap(), false).unwrap(),
            )
            .unwrap();
        }
        if mode == 2 {
            fs::hard_link(&path, f.0.join("alias")).unwrap();
        }
        let before = fs::read(&path).unwrap();
        assert!(Store::open(&f.0).is_err());
        assert_eq!(fs::read(path).unwrap(), before);
    }
}
#[test]
fn rotated_refresh_is_saved_before_downstream_failure_and_identity_never_switches() {
    let f = Fixture::new();
    let mut store = Store::open(&f.0).unwrap();
    store.save_authorization(authorized()).unwrap();
    let net = Mock::new(vec![
        (
            Endpoint::Token,
            200,
            json!({"access_token":"msa-secret","refresh_token":"rotated-secret"}),
        ),
        (Endpoint::Xbox, 503, json!({"Message":"private-secret"})),
    ]);
    assert!(
        store
            .authenticate(ID, &net, &Cancellation::default())
            .is_err()
    );
    drop(store);
    let mut store = Store::open(&f.0).unwrap();
    assert_eq!(store.data.accounts[0].refresh.expose(), "rotated-secret");
    let mut replies = vec![(Endpoint::Token, 200, json!({"access_token":"msa-secret"}))];
    replies.extend(chain());
    replies.last_mut().unwrap().2["id"] = json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert!(
        store
            .authenticate(ID, &Mock::new(replies), &Cancellation::default())
            .is_err()
    );
    assert_eq!(store.profiles()[0].id, ID);
}
#[test]
fn successful_refresh_rechecks_ownership_and_failed_save_keeps_previous_ciphertext() {
    let f = Fixture::new();
    let mut store = Store::open(&f.0).unwrap();
    store.save_authorization(authorized()).unwrap();
    let mut replies = vec![(
        Endpoint::Token,
        200,
        json!({"access_token":"msa-secret","refresh_token":"new-refresh"}),
    )];
    replies.extend(chain());
    let identity = store
        .authenticate(ID, &Mock::new(replies), &Cancellation::default())
        .unwrap();
    assert!(identity.is_current());
    let path = f.0.join(FILE);
    let before = fs::read(&path).unwrap();
    let _reader = options().share_mode(FILE_SHARE_READ).open(&path).unwrap();
    assert!(store.forget(ID).is_err());
    assert_eq!(store.profiles().len(), 1);
    assert_eq!(fs::read(&path).unwrap(), before);
}
#[test]
fn game_directories_and_linked_account_locks_are_rejected() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("versions")).unwrap();
    assert!(Store::open(&f.0).is_err());
    assert!(!f.0.join("accounts.lock").exists());
    let f = Fixture::new();
    fs::write(f.0.join("outside"), b"preserve").unwrap();
    fs::hard_link(f.0.join("outside"), f.0.join("accounts.lock")).unwrap();
    assert!(Store::open(&f.0).is_err());
    assert_eq!(fs::read(f.0.join("outside")).unwrap(), b"preserve");
}
