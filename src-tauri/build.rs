/// Default backend URL, from `.env` at the repo root (`LUNORX_BACKEND_URL=...`), else the production API.
/// Baked into the binary at build time. At run time `LUNORX_BACKEND_URL` in the environment still overrides it.
fn backend_default() -> String {
    const PRODUCTION: &str = "https://api.lunorx.xyz";
    println!("cargo:rerun-if-changed=../.env");
    println!("cargo:rerun-if-env-changed=LUNORX_BACKEND_URL");
    let from_file = std::fs::read_to_string("../.env").ok().and_then(|text| {
        text.lines().find_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            (key.trim() == "LUNORX_BACKEND_URL").then(|| value.trim().trim_matches('"').to_owned())
        })
    });
    let value = from_file
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| PRODUCTION.to_owned());
    // Only plain URL characters: the value is embedded in source as a string literal by env!().
    if value
        .bytes()
        .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\')
    {
        value
    } else {
        PRODUCTION.to_owned()
    }
}

fn main() {
    println!(
        "cargo:rustc-env=LUNORX_DEFAULT_BACKEND_URL={}",
        backend_default()
    );
    println!("cargo:rerun-if-env-changed=MPV_LIB_DIR");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let directory = std::env::var("MPV_LIB_DIR").ok().or_else(|| {
            ["/opt/homebrew/opt/mpv/lib", "/usr/local/opt/mpv/lib"]
                .into_iter()
                .find(|dir| std::path::Path::new(dir).join("libmpv.dylib").is_file())
                .map(str::to_owned)
        });
        if let Some(directory) = directory {
            println!("cargo:rustc-link-search=native={directory}");
            println!("cargo:rustc-link-arg=-Wl,-rpath,{directory}");
        }
    }
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "library_history",
            "library_clear_history",
            "discovery_feed",
            "discovery_genres",
            "fonts_installed",
            "discord_settings_get",
            "discord_settings_set",
            "discord_preview",
            "search_suggestions",
            "library_likes",
            "library_set_like",
            "library_artist",
            "library_album",
            "search_query",
            "search_cancel",
            "providers_list",
            "provider_credential_set",
            "provider_credential_status",
            "provider_credential_clear",
            "youtube_connect",
            "youtube_connect_finish",
            "youtube_connection_status",
            "youtube_import",
            "youtube_like_push_get",
            "youtube_like_push_set",
            "youtube_disconnect",
            "sync_sign_in",
            "sync_sign_out",
            "sync_discard_pending",
            "sync_status",
            "sync_create_local_playlist",
            "sync_add_local_entry",
            "sync_upload_playlist",
            "sync_list_local_playlists",
            "sync_playlist_entries",
            "sync_delete_local_playlist",
            "sync_rename_local_playlist",
            "sync_move_local_entry",
            "telemetry_get",
            "telemetry_set",
            "telemetry_record",
            "telemetry_flush",
            "telemetry_delete",
            "remote_start",
            "remote_stop",
            "remote_send",
            "stats_local",
            "stats_report",
            "stats_clear_local",
            "stats_set_cloud",
            "stats_cloud_sync",
            "stats_cloud_clear",
            "sync_remove_local_entry",
            "sync_flush_pending",
            "sync_resolve_conflict",
            "player_audio_settings",
            "player_set_audio_settings",
            "player_state",
            "player_progress",
            "player_track_info",
            "player_load",
            "player_play",
            "player_pause",
            "player_resume",
            "player_stop",
            "player_skip",
            "player_set_mode",
            "player_queue_edit",
            "player_seek",
            "player_set_volume",
            "player_observe",
            "player_open_external",
            "offline_status",
            "offline_keep",
            "offline_cancel",
            "offline_remove",
            "offline_set_limit",
            "lrc_load",
            "lrc_save",
            "lrc_delete",
            "lrc_reveal",
            "lrc_list",
            "lyrics_source_fetch",
            "lyrics_source_search",
            "lyrics_source_bind",
            "lyrics_source_unbind",
            "lyrics_settings_get",
            "lyrics_settings_set",
        ]),
    ))
    .unwrap()
}
