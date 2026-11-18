// SPDX-License-Identifier: MPL-2.0
use super::*;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::VecDeque};
pub(crate) const ID: &str = "0123456789abcdef0123456789abcdef";
pub(crate) struct Mock {
    pub replies: RefCell<VecDeque<(Endpoint, u32, Value)>>,
    pub requests: RefCell<Vec<Endpoint>>,
}
impl Mock {
    pub fn new(replies: Vec<(Endpoint, u32, Value)>) -> Self {
        Self {
            replies: RefCell::new(replies.into()),
            requests: RefCell::new(vec![]),
        }
    }
}
impl Transport for Mock {
    fn request(&self, req: Request<'_>, cancel: &Cancellation) -> Result<Reply> {
        cancel.check()?;
        self.requests.borrow_mut().push(req.endpoint);
        let (endpoint, status, body) = self
            .replies
            .borrow_mut()
            .pop_front()
            .expect("Unexpected auth request");
        assert_eq!(endpoint, req.endpoint);
        if req.endpoint.is_get() {
            assert_eq!(req.bearer.unwrap().expose(), "minecraft-secret");
            assert!(req.body.is_empty());
        } else {
            assert!(req.bearer.is_none());
        }
        Ok(Reply {
            status,
            body: Zeroizing::new(serde_json::to_vec(&body).unwrap()),
        })
    }
}
pub(crate) fn chain() -> Vec<(Endpoint, u32, Value)> {
    vec![
        (Endpoint::Xbox, 200, json!({"Token":"xbox-secret"})),
        (
            Endpoint::Xsts,
            200,
            json!({"Token":"xsts-secret","DisplayClaims":{"xui":[{"uhs":"123","xid":"456"}]}}),
        ),
        (
            Endpoint::Minecraft,
            200,
            json!({"access_token":"minecraft-secret","expires_in":86400}),
        ),
        (
            Endpoint::Entitlements,
            200,
            json!({"items":[{"name":"game_minecraft"}]}),
        ),
        (
            Endpoint::Profile,
            200,
            json!({"id":ID,"name":"TestPlayer","skins":[]}),
        ),
    ]
}
fn device_reply() -> (Endpoint, u32, Value) {
    (
        Endpoint::Device,
        200,
        json!({"device_code":"private-device-secret","user_code":"ABCD1234","verification_uri":"https://www.microsoft.com/link","expires_in":900,"interval":5}),
    )
}
#[test]
fn device_poll_respects_interval_slow_down_and_single_use_authorization() {
    let mut replies = vec![
        device_reply(),
        (
            Endpoint::Token,
            400,
            json!({"error":"authorization_pending"}),
        ),
        (Endpoint::Token, 400, json!({"error":"slow_down"})),
        (
            Endpoint::Token,
            200,
            json!({"access_token":"msa-secret","refresh_token":"refresh-secret"}),
        ),
    ];
    replies.extend(chain());
    let net = Mock::new(replies);
    let cancel = Cancellation::default();
    let mut device = DeviceFlow::begin(&net, &cancel).unwrap();
    let prompt = serde_json::to_string(device.prompt()).unwrap();
    assert!(!prompt.contains("private-device"));
    assert!(matches!(device.poll(&net, &cancel).unwrap(), Poll::Waiting));
    assert_eq!(net.requests.borrow().len(), 1);
    device.next = Instant::now();
    assert!(matches!(device.poll(&net, &cancel).unwrap(), Poll::Waiting));
    device.next = Instant::now();
    assert!(matches!(device.poll(&net, &cancel).unwrap(), Poll::Waiting));
    assert_eq!(device.interval, Duration::from_secs(10));
    device.next = Instant::now();
    let Poll::Authorized(a) = device.poll(&net, &cancel).unwrap() else {
        panic!()
    };
    assert_eq!(a.identity.profile.id, ID);
    assert_eq!(a.refresh.expose(), "refresh-secret");
    assert!(a.identity.is_current());
    assert!(device.poll(&net, &cancel).is_err());
    assert!(net.replies.borrow().is_empty());
}
#[test]
fn cancelled_expired_and_denied_device_flows_never_request_a_game_identity() {
    for mode in 0..3 {
        let net = Mock::new(vec![
            device_reply(),
            (
                Endpoint::Token,
                400,
                json!({"error":"authorization_declined","error_description":"raw-private-secret"}),
            ),
        ]);
        let cancel = Cancellation::default();
        let mut flow = DeviceFlow::begin(&net, &cancel).unwrap();
        flow.next = Instant::now();
        if mode == 0 {
            cancel.cancel();
        }
        if mode == 1 {
            flow.deadline = Instant::now();
        }
        let error = match flow.poll(&net, &cancel) {
            Err(e) => e,
            _ => panic!("Expected refusal"),
        };
        assert!(!error.contains("raw-private"));
        assert!(!net.requests.borrow().contains(&Endpoint::Xbox));
    }
}
#[test]
fn ownership_and_profile_are_required_and_errors_never_echo_service_credentials() {
    for mode in 0..4 {
        let mut replies = chain();
        match mode {
            0 => replies[3].2 = json!({"items":[]}),
            1 => replies[4].2 = json!({"id":"bad","name":"Test"}),
            2 => {
                replies[1].1 = 401;
                replies[1].2 = json!({"XErr":2148916238u64,"Message":"raw-private-secret"});
            }
            _ => replies[2].2 = json!({"access_token":"header\r\ninjection","expires_in":86400}),
        };
        let net = Mock::new(replies);
        let error = match minecraft(
            &Secret::new("msa-secret".into()).unwrap(),
            &net,
            &Cancellation::default(),
        ) {
            Err(e) => e,
            _ => panic!("Unexpected authorization"),
        };
        assert!(!error.contains("secret") && !error.contains("injection"));
    }
}
#[test]
fn form_values_are_encoded_and_credentials_have_redacted_debug() {
    let encoded = form(&[("refresh_token", "a&b=+/%")]);
    assert_eq!(
        std::str::from_utf8(&encoded).unwrap(),
        "refresh_token=a%26b%3D%2B%2F%25"
    );
    assert_eq!(
        format!("{:?}", Secret::new("private-secret".into()).unwrap()),
        "[REDACTED]"
    );
    assert!(Secret::new("\r\n".into()).is_err());
    assert_eq!(
        Endpoint::Profile.destination().0,
        "api.minecraftservices.com"
    );
}
#[test]
fn device_prompt_rejects_unrecognized_verification_destination() {
    let mut reply = device_reply();
    reply.2["verification_uri"] = json!("https://www.microsoft.com.evil.invalid/link");
    assert!(DeviceFlow::begin(&Mock::new(vec![reply]), &Cancellation::default()).is_err());
}
