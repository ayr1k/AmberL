// SPDX-License-Identifier: MPL-2.0
//! Explicit developer fixture: owned server and two no-TUN peers on one Windows host.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ambrum::{
        multiplayer::{self, Address, room, server},
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
    if ![4, 7].contains(&a.len()) || a[3] != "--accept-eula" {
        return Err(
            "Usage: multiplayer_smoke APP JAVA VERSION --accept-eula [AUTH_APP GAME OS] (new disposable server only)"
                .into(),
        );
    }
    let app = PathBuf::from(&a[0]);
    let java = PathBuf::from(&a[1]);
    let port = 27565;
    let record = server::create(
        &app,
        "Multiplayer isolated QA",
        &a[2],
        port,
        true,
        &Cancellation::default(),
    )?;
    println!("Created isolated server {}", record.id);
    room::install(&app, &Cancellation::default())?;
    println!("Verified official EasyTier {}", room::VERSION);
    struct Relay(std::process::Child);
    impl Drop for Relay {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let probe = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let relay_port = probe.local_addr()?.port();
    drop(probe);
    let mut relay = Relay(
        Command::new(app.join("network-components/easytier-2.6.4.exe"))
            .args([
                "--no-tun",
                "--listeners",
                &format!("tcp://127.0.0.1:{relay_port}"),
                "--rpc-portal",
                "127.0.0.1:0",
                "--network-name",
                "AmberL-isolated-test-relay",
                "--console-log-level",
                "off",
            ])
            .env("ET_NETWORK_SECRET", "isolated-relay-no-account-data")
            .current_dir(&app)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()?,
    );
    thread::sleep(Duration::from_secs(1));
    if relay.0.try_wait()?.is_some() {
        return Err("Local relay failed".into());
    }
    let server_cancel = Cancellation::default();
    let c = server_cancel.clone();
    let p = app.clone();
    let j = java.clone();
    let id = record.id.clone();
    let (_commands, rx) = mpsc::sync_channel(8);
    let (events, er) = mpsc::channel();
    let server_thread = thread::spawn(move || {
        server::run(&p, &id, &j, 2048, &c, &rx, |e| {
            let _ = events.send(e);
        })
    });
    let room_cancel = Cancellation::default();
    let mut rooms = vec![];
    let mut client_joined = false;
    let result = (|| -> ambrum::Result<()> {
        let until = Instant::now() + Duration::from_secs(150);
        let mut ready = false;
        while Instant::now() < until {
            if server_thread.is_finished() {
                return Err("Server ended before readiness".into());
            }
            if let Ok(event) = er.recv_timeout(Duration::from_secs(1)) {
                match event {
                    server::Event::Ready => {
                        ready = true;
                        break;
                    }
                    server::Event::Line(s) if s.contains("ERROR") || s.contains("Exception") => {
                        eprintln!("{s}");
                    }
                    _ => {}
                }
            }
        }
        if !ready {
            return Err("Server readiness timeout".into());
        }
        let local = Address::parse(&format!("127.0.0.1:{port}"))?;
        let deadline = Instant::now() + Duration::from_secs(30);
        let s = loop {
            match multiplayer::probe(&local) {
                Ok(s) => break s,
                Err(e) => {
                    if Instant::now() >= deadline {
                        return Err(e);
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            }
        };
        println!("Direct Minecraft status: {}", s.version);
        let invite = room::Invite::host(port, &format!("tcp://127.0.0.1:{relay_port}"))?;
        for guest in [None, Some(27567)] {
            let c = room_cancel.clone();
            let p = app.clone();
            let i = invite.clone();
            rooms.push(thread::spawn(move || room::run(&p, i, guest, &c, |_| {})));
        }
        let until = Instant::now() + Duration::from_secs(60);
        let target = Address::parse("127.0.0.1:27567")?;
        let mut ok = false;
        while Instant::now() < until {
            if rooms.iter().any(|h| h.is_finished()) {
                return Err("Room process exited before reachability".into());
            }
            if let Ok(s) = multiplayer::probe(&target) {
                println!("Through two EasyTier peers: {}", s.version);
                ok = true;
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }
        if !ok {
            return Err("Room status deadline; no cross-peer reachability established".into());
        }
        if a.len() == 7 {
            let auth = PathBuf::from(&a[4]);
            let game = PathBuf::from(&a[5]);
            let profiles = ambrum::secure::Store::open(&auth)?.profiles();
            if profiles.len() != 1 {
                return Err(
                    "Fixture requires exactly one explicitly authorized test account".into(),
                );
            }
            let account = profiles[0].id.clone();
            let version = a[2].clone();
            let os = a[6].clone();
            let j = java.clone();
            let close = Cancellation::default();
            let c = close.clone();
            let target2 = target.clone();
            let client = thread::spawn(move || {
                ambrum::launch::run_join(
                    &ambrum::launch::Selection {
                        app_directory: &auth,
                        game_directory: &game,
                        version: &version,
                        java_executable: &j,
                        account_id: &account,
                        max_memory_mib: 4096,
                        os_version: &os,
                    },
                    Some(&target2),
                    &c,
                    |e| {
                        if matches!(e, ambrum::launch::Event::Started { .. }) {
                            println!("Authenticated game process started");
                        }
                    },
                )
            });
            let until = Instant::now() + Duration::from_secs(120);
            while Instant::now() < until && !client.is_finished() {
                if multiplayer::probe(&target).is_ok_and(|s| s.players == 1) {
                    client_joined = true;
                    println!(
                        "Online-mode server reports one authenticated connected player through room"
                    );
                    break;
                }
                thread::sleep(Duration::from_secs(1));
            }
            if client_joined {
                thread::sleep(Duration::from_secs(8));
            }
            close.cancel();
            let exit = client.join().map_err(|_| "Client thread failed")??;
            println!("Client normal exit: {:?}", exit.exit_code);
            if !client_joined || !exit.success {
                return Err("Client join or normal exit not verified".into());
            }
        }
        Ok(())
    })();
    room_cancel.cancel();
    for h in rooms {
        let _ = h.join();
    }
    server_cancel.cancel();
    let exit = server_thread
        .join()
        .map_err(|_| "Server test thread failed")??;
    println!("Server graceful exit: {exit}");
    result?;
    if exit != 0 {
        return Err("Server exit failure".into());
    }
    let world = app.join("servers").join(&record.id).join("world/level.dat");
    if !world.is_file() {
        return Err("World save missing".into());
    }
    // Reopen this exact registered directory and saved world, rather than creating another server.
    let restart_cancel = Cancellation::default();
    let c = restart_cancel.clone();
    let p = app.clone();
    let j = java.clone();
    let id = record.id.clone();
    let (_commands, rx) = mpsc::sync_channel(8);
    let (events, er) = mpsc::channel();
    let restarted = thread::spawn(move || {
        server::run(&p, &id, &j, 2048, &c, &rx, |e| {
            let _ = events.send(e);
        })
    });
    let deadline = Instant::now() + Duration::from_secs(150);
    let mut reloaded = false;
    let mut recreated = false;
    while Instant::now() < deadline && !restarted.is_finished() {
        match er.recv_timeout(Duration::from_secs(1)) {
            Ok(server::Event::Ready) => {
                reloaded = true;
                break;
            }
            Ok(server::Event::Line(s)) if s.contains("No existing world data") => recreated = true,
            _ => {}
        }
    }
    restart_cancel.cancel();
    let restart_exit = restarted.join().map_err(|_| "Restart thread failed")??;
    if !reloaded || recreated || restart_exit != 0 || !world.is_file() {
        return Err("Same-world reload or graceful second stop failed".into());
    }
    println!("Same registered world reloaded and stopped normally");
    println!(
        "{}",
        serde_json::json!({"passed":true,"server_id":record.id,"version":a[2],"direct_status":true,"two_local_peers_status":true,"server_exit":exit,"world_saved":true,"same_world_reloaded":true,"two_physical_devices":false,"game_client_joined":client_joined})
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows fixture only");
}
