// SPDX-License-Identifier: MPL-2.0
//! Disposable localhost-only FRP relay, actual Minecraft query and reconnect check.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ambrum::{
        download::{Algorithm, Artifact, Cache, Transport},
        multiplayer::{self, Address, frp, server},
        task::Cancellation,
    };
    use std::{
        os::windows::process::CommandExt,
        path::PathBuf,
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 7 || a[6] != "--accept-eula" {
        return Err(
            "frp_smoke APP FRPS CA_PEM KEY_PEM JAVA GAME --accept-eula (disposable local fixture)"
                .into(),
        );
    }
    let app = PathBuf::from(&a[0]);
    std::fs::create_dir_all(&app)?;
    let app = std::fs::canonicalize(app)?;
    let executable = std::fs::canonicalize(&a[1])?;
    let cert = std::fs::canonicalize(&a[2])?;
    let key = std::fs::canonicalize(&a[3])?;
    let java = std::fs::canonicalize(&a[4])?;
    struct LocalOfficial(PathBuf);
    impl Transport for LocalOfficial {
        fn open(&self, _: &str) -> ambrum::Result<Box<dyn std::io::Read>> {
            Ok(Box::new(
                std::fs::File::open(&self.0).map_err(|e| e.to_string())?,
            ))
        }
    }
    let executable = Cache::open(&app)?
        .fetch(
            &Artifact {
                url: "https://github.com/fatedier/frp/releases/download/v0.72.0/frps.exe".into(),
                size: 21318144,
                algorithm: Algorithm::Sha256,
                digest: "ccee79ff6da822e2577ffb668815793a358fecad2fd9b6db26641b2d749516f6".into(),
            },
            &LocalOfficial(executable),
            &Cancellation::default(),
            |_| {},
        )?
        .path;
    let ports = (0..3)
        .map(|_| std::net::TcpListener::bind(("127.0.0.1", 0)))
        .collect::<std::io::Result<Vec<_>>>()?;
    let port = ports[0].local_addr()?.port();
    let relay = ports[1].local_addr()?.port();
    let public = ports[2].local_addr()?.port();
    let record = server::create(
        &app,
        "FRP local QA",
        &a[5],
        port,
        true,
        &Cancellation::default(),
    )?;
    frp::install(&app, &Cancellation::default())?;
    let config = frp::Config {
        server: Address::parse(&format!("127.0.0.1:{relay}"))?,
        local_port: port,
        remote_port: public,
        certificate: cert.clone(),
    };
    frp::save(&app, &config, "amberl-disposable-frp-qa".into())?;
    let relay_config = app.join("qa-frps.json");
    std::fs::write(
        &relay_config,
        serde_json::to_vec(
            &serde_json::json!({"bindAddr":"127.0.0.1","proxyBindAddr":"127.0.0.1","bindPort":relay,
        "auth":{"method":"token","token":"amberl-disposable-frp-qa"},"transport":{"tls":{"certFile":cert,"keyFile":key,"force":true}},"log":{"to":"console","level":"error"}}),
        )?,
    )?;
    struct Process(std::process::Child);
    impl Drop for Process {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    struct Worker {
        cancel: Cancellation,
        handle: Option<thread::JoinHandle<ambrum::Result<i32>>>,
    }
    impl Worker {
        fn stop(&mut self) -> ambrum::Result<i32> {
            self.cancel.cancel();
            self.handle
                .take()
                .ok_or("Already stopped")?
                .join()
                .map_err(|_| "Worker panic")?
        }
    }
    impl Drop for Worker {
        fn drop(&mut self) {
            self.cancel.cancel();
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
        }
    }
    let game_cancel = Cancellation::default();
    let token = game_cancel.clone();
    let game_app = app.clone();
    let id = record.id.clone();
    let (_commands, rx) = mpsc::channel();
    let (events, observations) = mpsc::channel();
    drop(ports);
    let mut game = Worker {
        cancel: game_cancel,
        handle: Some(thread::spawn(move || {
            server::run(&game_app, &id, &java, 2048, &token, &rx, |e| {
                let _ = events.send(e);
            })
        })),
    };
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut ready = false;
    while Instant::now() < deadline {
        if matches!(
            observations.recv_timeout(Duration::from_millis(100)),
            Ok(server::Event::Ready)
        ) {
            ready = true;
            break;
        }
        if game.handle.as_ref().unwrap().is_finished() {
            break;
        }
    }
    if !ready {
        return Err("Minecraft server did not report readiness".into());
    }
    let spawn = || -> std::io::Result<Process> {
        Ok(Process(
            Command::new(&executable)
                .arg("-c")
                .arg(&relay_config)
                .current_dir(&app)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x08000000)
                .spawn()?,
        ))
    };
    let mut relay_process = spawn()?;
    let frp_cancel = Cancellation::default();
    let token = frp_cancel.clone();
    let frp_app = app.clone();
    let mut tunnel = Worker {
        cancel: frp_cancel,
        handle: Some(thread::spawn(move || {
            frp::run(&frp_app, &config, &token, |_| {})
        })),
    };
    let address = Address {
        host: "127.0.0.1".into(),
        port: public,
    };
    let wait = |wanted: bool| {
        let deadline = Instant::now() + Duration::from_secs(45);
        while Instant::now() < deadline {
            if multiplayer::probe(&address).is_ok() == wanted {
                return true;
            }
            thread::sleep(Duration::from_millis(300));
        }
        false
    };
    if !wait(true) {
        return Err("Minecraft query through FRP failed".into());
    }
    println!("PASS verified TLS FRP Minecraft query");
    relay_process.0.kill()?;
    relay_process.0.wait()?;
    if !wait(false) {
        return Err("Relay shutdown still appeared reachable".into());
    }
    relay_process = spawn()?;
    if !wait(true) {
        return Err("FRP did not reconnect after local relay restart".into());
    }
    println!("PASS same frpc reconnect after relay restart");
    if tunnel.stop()? != 0 || !wait(false) {
        return Err("FRP did not stop cleanly".into());
    }
    multiplayer::probe(&Address {
        host: "127.0.0.1".into(),
        port,
    })?;
    println!("PASS stopping tunnel leaves Minecraft server running");
    if game.stop()? != 0
        || !app
            .join("servers")
            .join(record.id)
            .join("world/level.dat")
            .is_file()
    {
        return Err("Minecraft graceful save failed".into());
    }
    drop(relay_process);
    println!(
        "PASS local FRP lifecycle/reconnect/independent stop/world save; not public-network or two-device acceptance"
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {}
