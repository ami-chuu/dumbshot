use chrono::Local;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use which::which;

const ICON_AREA: &[u8] = include_bytes!("../assets/icons/Area.svg");
const ICON_MONITOR: &[u8] = include_bytes!("../assets/icons/Monitor.svg");
const ICON_COPY: &[u8] = include_bytes!("../assets/icons/Copy.svg");
const ICON_EDIT: &[u8] = include_bytes!("../assets/icons/Edit.svg");
const ICON_SAVE: &[u8] = include_bytes!("../assets/icons/Save.svg");
const ICON_SAVENCOPY: &[u8] = include_bytes!("../assets/icons/SavenCopy.svg");
const WOFI_CONFIG: &[u8] = include_bytes!("../assets/wofi/config");
const WOFI_STYLE: &[u8] = include_bytes!("../assets/wofi/style.css");

fn ensure_config() -> PathBuf {
    let config_dir = dirs::config_dir().expect("Could not find config directory").join("dumbshot");
    let icons_dir = config_dir.join("icons");
    let wofi_dir = config_dir.join("wofi");

    let _ = fs::create_dir_all(&icons_dir);
    let _ = fs::create_dir_all(&wofi_dir);

    let write_if_not_exists = |path: PathBuf, data: &[u8]| {
        if !path.exists() {
            let _ = fs::write(path, data);
        }
    };

    write_if_not_exists(icons_dir.join("Area.svg"), ICON_AREA);
    write_if_not_exists(icons_dir.join("Monitor.svg"), ICON_MONITOR);
    write_if_not_exists(icons_dir.join("Copy.svg"), ICON_COPY);
    write_if_not_exists(icons_dir.join("Edit.svg"), ICON_EDIT);
    write_if_not_exists(icons_dir.join("Save.svg"), ICON_SAVE);
    write_if_not_exists(icons_dir.join("SavenCopy.svg"), ICON_SAVENCOPY);
    write_if_not_exists(wofi_dir.join("config"), WOFI_CONFIG);
    write_if_not_exists(wofi_dir.join("style.css"), WOFI_STYLE);

    config_dir
}

fn extract_label(input: &str) -> &str {
    input.rfind(':').map_or(input, |pos| &input[pos + 1..])
}

fn run_menu(prompt: &str, opts: &[&str], config_path: &PathBuf) -> Option<String> {
    let launcher = ["wofi", "rofi", "dmenu"]
        .into_iter()
        .find(|&bin| which(bin).is_ok())?;

    let mut cmd = Command::new(launcher);

    match launcher {
        "wofi" => {
            cmd.args(["--dmenu", "--allow-images", "--width", "200", "--no-cache", "--insensitive"]);
            cmd.arg("--prompt").arg(prompt);
            
            let base = config_path.join("wofi");
            let cfg = base.join("config");
            let style = base.join("style.css");
            if cfg.exists() { cmd.arg("--conf").arg(cfg); }
            if style.exists() { cmd.arg("--style").arg(style); }
        }
        "rofi" => { 
            cmd.args(["-dmenu", "-p", prompt, "-sort", "false", "-i"]); 
        }
        "dmenu" => { cmd.args(["-p", prompt, "-i"]); }
        _ => unreachable!(),
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;

    {
        let mut stdin = child.stdin.take()?;
        stdin.write_all(opts.join("\n").as_bytes()).ok()?;
    }

    let output = child.wait_with_output().ok()?;
    if !output.status.success() { return None; }

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if result.is_empty() { None } else { Some(result) }
}

fn get_monitors_list() -> Option<Vec<String>> {
    which("hyprctl").ok()?;
    let out = Command::new("hyprctl").args(["monitors", "-j"]).output().ok()?;
    let v: Value = serde_json::from_slice(&out.stdout).ok()?;
    
    let names: Vec<String> = v.as_array()?
        .iter()
        .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
        .collect();

    if names.is_empty() { None } else { Some(names) }
}

fn capture_screenshot(args: &[&str], path: &str) -> bool {
    thread::sleep(Duration::from_millis(200));
    Command::new("grim")
        .args(args)
        .arg(path)
        .status()
        .map_or(false, |s| s.success())
}

fn main() {
    if which("grim").is_err() || which("slurp").is_err() {
        eprintln!("Error: 'grim' and 'slurp' are required.");
        return;
    }

    let config_path = ensure_config();
    let icon_path = config_path.join("icons");
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    let main_opts = [
        &format!("img:{}/Area.svg:text:Area", icon_path.display()),
        &format!("img:{}/Monitor.svg:text:Monitor", icon_path.display()),
        "text:All",
        "text:Cancel",
    ];

    let choice_raw = run_menu("Screenshot", &main_opts, &config_path).unwrap_or_else(|| "Cancel".into());
    let choice = extract_label(&choice_raw);
    if choice == "Cancel" { return; }

    let tmp_path = std::env::temp_dir().join(format!("shot-{}.png", Local::now().format("%Y%m%d%H%M%S")));
    let tmp_str = tmp_path.to_string_lossy();

    let success = match choice {
        "Area" => {
            let geom = Command::new("slurp").output().ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            
            if let Some(g) = geom { capture_screenshot(&["-g", &g], &tmp_str) } else { false }
        }
        "Monitor" => {
            if let Some(monitors) = get_monitors_list() {
                let m_opts: Vec<&str> = monitors.iter().map(|s| s.as_str()).collect();
                run_menu("Choose monitor", &m_opts, &config_path)
                    .map_or(false, |m| capture_screenshot(&["-o", &m], &tmp_str))
            } else {
                capture_screenshot(&[], &tmp_str)
            }
        }
        "All" => capture_screenshot(&[], &tmp_str),
        _ => false,
    };

    if !success {
        let _ = Command::new("notify-send").arg("Screenshot failed").status();
        let _ = fs::remove_file(&tmp_path);
        return;
    }

    let actions = [
        &format!("img:{}/Save.svg:text:Save", icon_path.display()),
        &format!("img:{}/Copy.svg:text:Copy", icon_path.display()),
        &format!("img:{}/Edit.svg:text:Edit", icon_path.display()),
        &format!("img:{}/SavenCopy.svg:text:Save&Copy", icon_path.display()),
        "text:Cancel",
    ];

    let act_raw = run_menu("Action", &actions, &config_path).unwrap_or_else(|| "Cancel".into());
    let act = extract_label(&act_raw);

    match act {
        "Save" | "Save&Copy" => {
            let mut dst = dirs::picture_dir().unwrap_or(home.join("Pictures")).join("Screenshots");
            if fs::create_dir_all(&dst).is_err() { return; }

            dst.push(format!("screenshot_{}.png", Local::now().format("%Y-%m-%d_%H-%M-%S")));
            
            if fs::copy(&tmp_path, &dst).is_ok() {
                if act == "Save&Copy" {
                    let _ = Command::new("wl-copy").arg("--type").arg("image/png").stdin(fs::File::open(&dst).unwrap()).status();
                }
                let _ = Command::new("notify-send").arg("Saved").arg(dst.to_string_lossy().as_ref()).status();
            }
        }
        "Copy" => {
            if let Ok(file) = fs::File::open(&tmp_path) {
                let _ = Command::new("wl-copy").arg("--type").arg("image/png").stdin(file).status();
                let _ = Command::new("notify-send").arg("Copied").status();
            }
        }
        "Edit" => {
            let editor = if which("satty").is_ok() { "satty" } else { "xdg-open" };
            let mut cmd = Command::new(editor);
            if editor == "satty" { cmd.arg("-f"); }
            let _ = cmd.arg(&*tmp_str).status();
        }
        _ => {}
    }

    let _ = fs::remove_file(&tmp_path);
}
