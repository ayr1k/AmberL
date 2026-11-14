// SPDX-License-Identifier: MPL-2.0
//! Creates a disposable owned world; verifies readiness, console, save and reload.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ambrum::{
        loader,
        multiplayer::{self, Address, server},
        task::Cancellation,
    };
    use std::{
        path::PathBuf,
        sync::mpsc,
        time::{Duration, Instant},
    };
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 6 || args[5] != "--accept-eula" {
        return Err("server_smoke APP JAVA GAME KIND LOADER_VERSION --accept-eula (disposable new world only)".into());
    }
    let app = PathBuf::from(&args[0]);
    std::fs::create_dir_all(&app)?;
    let java = PathBuf::from(&args[1]);
    let kind = &args[3];
    let reservation = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let port = reservation.local_addr()?.port();
    let choice = if kind == "Vanilla" {
        None
    } else {
        Some(loader::Choice {
            kind: kind.clone(),
            version: args[4].clone(),
        })
    };
    let mut record = server::create_with_loader(
        &app,
        &server::Create {
            name: format!("{kind} isolated QA"),
            version: args[2].clone(),
            port,
            eula: true,
            loader: choice,
            java: Some(java.clone()),
        },
        &Cancellation::default(),
    )?;
    drop(reservation);
    println!("CREATED {} {} {}", record.id, record.version, kind);
    for round in 0..3 {
        let cancel = Cancellation::default();
        let token = cancel.clone();
        let p = app.clone();
        let id = record.id.clone();
        let j = java.clone();
        let (commands, rx) = mpsc::channel();
        let (tx, events) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            server::run(&p, &id, &j, 2048, &token, &rx, |e| {
                let _ = tx.send(e);
            })
        });
        let deadline = Instant::now() + Duration::from_secs(240);
        let mut ready = false;
        let mut listed = false;
        let mut probe_ok = false;
        while Instant::now() < deadline && !handle.is_finished() {
            if let Ok(e) = events.recv_timeout(Duration::from_millis(100)) {
                match e {
                    server::Event::Ready => {
                        ready = true;
                        for _ in 0..20 {
                            probe_ok = multiplayer::probe(&Address {
                                host: "127.0.0.1".into(),
                                port: record.port,
                            })
                            .is_ok();
                            if probe_ok {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(250));
                        }
                        commands.send("list".into())?;
                    }
                    server::Event::Line(line) => {
                        println!("{line}");
                        if ready && line.contains("players online") {
                            listed = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        cancel.cancel();
        let result = handle.join().map_err(|_| "Server worker panicked")?;
        for e in events.try_iter() {
            if let server::Event::Line(l) = e {
                println!("{l}");
            }
        }
        let code = result?;
        let saved = app
            .join("servers")
            .join(&record.id)
            .join("world/level.dat")
            .is_file();
        println!(
            "ROUND {round}: ready={ready} status={probe_ok} console={listed} exit={code} saved={saved}"
        );
        if !ready || !probe_ok || !listed || code != 0 || !saved {
            return Err("Server lifecycle check failed".into());
        }
        if round == 1 {
            let original = app.join("servers").join(&record.id).join("world/level.dat");
            let before = std::fs::read(&original)?;
            let backup = server::backup::create(&app, &record.id, &Cancellation::default())?;
            let restored = server::backup::restore(
                &app,
                &backup,
                "Recovered QA world",
                record.port,
                &Cancellation::default(),
            )?;
            if std::fs::read(
                app.join("servers")
                    .join(&restored.id)
                    .join("world/level.dat"),
            )? != before
                || std::fs::read(&original)? != before
            {
                return Err("Snapshot world differs or original changed".into());
            }
            server::set_archived(&app, &record.id, true)?;
            if server::list(&app)?.iter().any(|s| s.id == record.id) {
                return Err("Archive still launchable".into());
            }
            server::set_archived(&app, &record.id, false)?;
            println!(
                "BACKUP_RESTORED original={} copy={} preserved=true",
                record.id, restored.id
            );
            record = restored;
        }
    }
    println!(
        "PASS: owned server startup, query, console, save, same-world reload and verified backup restoration; no remote-device claim"
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {}
