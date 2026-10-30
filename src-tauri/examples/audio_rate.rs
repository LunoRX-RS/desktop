use libmpv2::Mpv;
fn main() {
    // BlackHole 2ch
    let mpv = Mpv::with_initializer(|init| {
        init.set_option("config", "no")?;
        init.set_option("ao", "coreaudio")?;
        init.set_option("audio-device", "coreaudio/BlackHole2ch_UID")?;
        init.set_option("vid", "no")?;
        init.set_option("terminal", "no")
    })
    .unwrap();
    let _ = mpv.command("loadfile", &["/System/Library/Sounds/Ping.aiff", "replace"]);
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let sr: Option<i64> = mpv.get_property("audio-out-params/samplerate").ok();
        if sr.is_some() {
            println!("BlackHole auto out_sr={:?}", sr);
            break;
        }
    }
    // now force 96k
    let mpv2 = Mpv::with_initializer(|init| {
        init.set_option("config", "no")?;
        init.set_option("ao", "coreaudio")?;
        init.set_option("audio-device", "coreaudio/BlackHole2ch_UID")?;
        init.set_option("audio-samplerate", "96000")?;
        init.set_option("vid", "no")?;
        init.set_option("terminal", "no")
    })
    .unwrap();
    let _ = mpv2.command("loadfile", &["/System/Library/Sounds/Ping.aiff", "replace"]);
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let sr: Option<i64> = mpv2.get_property("audio-out-params/samplerate").ok();
        if sr.is_some() {
            println!("BlackHole forced96 out_sr={:?}", sr);
            break;
        }
    }
}
