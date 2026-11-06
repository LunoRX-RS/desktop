//! Manual Core Audio rate check: cargo run --example audio_rate -- <CoreAudio device UID> [sample rate]
//! Plays a short system sound on that device and prints the output rate mpv reports. Without a rate it uses Automatic.
use libmpv2::Mpv;

fn play(device: &str, rate: Option<&str>) -> Result<Option<i64>, libmpv2::Error> {
    let mpv = Mpv::with_initializer(|init| {
        init.set_option("config", "no")?;
        init.set_option("ao", "coreaudio")?;
        init.set_option("audio-device", format!("coreaudio/{device}").as_str())?;
        if let Some(rate) = rate {
            init.set_option("audio-samplerate", rate)?;
        }
        init.set_option("vid", "no")?;
        init.set_option("terminal", "no")
    })?;
    mpv.command("loadfile", &["/System/Library/Sounds/Ping.aiff", "replace"])?;
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if let Ok(sr) = mpv.get_property::<i64>("audio-out-params/samplerate") {
            return Ok(Some(sr));
        }
    }
    Ok(None)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(device) = args.next() else {
        eprintln!("usage: audio_rate <CoreAudio device UID> [sample rate]");
        std::process::exit(2);
    };
    let rate = args.next();
    match play(&device, rate.as_deref()) {
        Ok(Some(sr)) => println!("device {device} out_sr={sr}"),
        Ok(None) => eprintln!("device {device}: no output rate reported"),
        Err(error) => {
            eprintln!("device {device}: mpv failed: {error}");
            std::process::exit(1);
        }
    }
}
