"use client";
import { useEffect, useState } from "react";
import { Button } from "../../components/ui";
import { playerAudioDevices, playerAudioSettings, playerSetAudioDevice, playerSetAudioSettings, type AudioDevicesDto, type AudioSettings, type AudioSettingsDto } from "../../services/tauri/player";
import { AUDIO_CHANGED, saveAudioDevice, saveAudioSettings } from "../../lib/audioSettings";
import { AudioEffectsPanel } from "./AudioEffects";
import { setSleepTimer, useSleepTimer } from "../../lib/sleepTimer";
import { usePlatform } from "../../lib/platform";
import { FIELD, Inset, NoteRow, SettingRow } from "./rows";

const LOUDNESS_HINT: Record<AudioSettings["loudness"], string> = {
  off: "Plays the track as it comes from YouTube. Quiet tracks stay quiet.",
  balanced: "Evens out quiet and loud parts so tracks sound closer in volume.",
  loud: "Balanced, plus 4 dB more gain with a limiter to avoid clipping. May sound compressed on quiet tracks.",
};

const OUTPUT_HINT = {
  mac: "macOS uses shared CoreAudio. Device hardware format remains controlled by Audio MIDI Setup.",
  linux: "Linux uses the first output mpv can open, usually PipeWire, then PulseAudio, then ALSA. The sound server decides the device format.",
  other: "The system audio output decides the device hardware format.",
} as const;

export function AudioSettingsPanel() {
  const platform = usePlatform();
  const [value, setValue] = useState<AudioSettingsDto | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [devices, setDevices] = useState<AudioDevicesDto | null>(null);
  const [deviceError, setDeviceError] = useState(false);
  // The list is read when the panel opens and again every few seconds, so a headset plugged in later appears.
  useEffect(() => {
    let active = true;
    const read = () => playerAudioDevices().then((d) => { if (active) { setDevices(d); setDeviceError(false); } }, () => { if (active) setDeviceError(true); });
    void read();
    const timer = window.setInterval(read, 4000);
    return () => { active = false; window.clearInterval(timer); };
  }, []);
  async function chooseDevice(id: string) {
    setDeviceError(false);
    try { const next = await playerSetAudioDevice(id); setDevices(next); saveAudioDevice(next.selected); } catch { setDeviceError(true); }
  }
  useEffect(() => {
    let active = true;
    playerAudioSettings().then(v => { if (active) setValue(v); }).catch(() => { if (active) setError(true); });
    // The macOS Sound menu changes the same settings.
    const changed = (e: Event) => { if (active) setValue((e as CustomEvent<AudioSettingsDto>).detail); };
    window.addEventListener(AUDIO_CHANGED, changed);
    return () => { active = false; window.removeEventListener(AUDIO_CHANGED, changed); };
  }, []);
  async function update(settings: AudioSettings) {
    setBusy(true); setError(false);
    try { const next = await playerSetAudioSettings(settings); setValue(next); saveAudioSettings(next.settings); } catch { setError(true); }
    finally { setBusy(false); }
  }
  async function refresh() {
    setBusy(true); setError(false);
    try { setValue(await playerAudioSettings()); } catch { setError(true); }
    finally { setBusy(false); }
  }
  const endsAt = useSleepTimer((t) => t.endsAt);
  const disabled = busy || !value?.available;
  const output = value?.output;
  return <>
  <Inset id="settings-audio" title="Audio quality and output">
    <SettingRow label="Stream quality" htmlFor="audio-quality" hint="YouTube target bitrate. Uses the lowest available stream if none meets the limit.">
      <select id="audio-quality" className={FIELD} disabled={disabled} value={value?.settings.quality ?? "high"} onChange={e => value && update({ ...value.settings, quality: e.target.value as AudioSettings["quality"] })}>
        <option value="low">Low · up to 64 kbps</option><option value="normal">Normal · up to 128 kbps</option><option value="high">High · best available</option>
      </select>
    </SettingRow>
    <SettingRow label="PCM output" htmlFor="audio-pcm" hint="24-bit sources fit in 32-bit PCM. Output format does not add source detail; the device may negotiate another format.">
      <select id="audio-pcm" className={FIELD} disabled={disabled} value={value?.settings.pcmFormat ?? "float32"} onChange={e => value && update({ ...value.settings, pcmFormat: e.target.value as AudioSettings["pcmFormat"] })}>
        <option value="int16">16-bit integer</option><option value="int32">32-bit integer (includes 24-bit)</option><option value="float32">32-bit float</option>
      </select>
    </SettingRow>
    <SettingRow label="Sample rate" htmlFor="audio-rate" hint="Automatic follows the source where supported. Other rates resample audio.">
      <select id="audio-rate" className={FIELD} disabled={disabled} value={value?.settings.sampleRate ?? "auto"} onChange={e => value && update({ ...value.settings, sampleRate: e.target.value as AudioSettings["sampleRate"] })}>
        <option value="auto">Automatic</option><option value="hz44100">44.1 kHz</option><option value="hz48000">48 kHz</option><option value="hz96000">96 kHz</option><option value="hz192000">192 kHz</option>
      </select>
    </SettingRow>
    <SettingRow label="Loudness" htmlFor="audio-loudness" hint={LOUDNESS_HINT[value?.settings.loudness ?? "balanced"]}>
      <select id="audio-loudness" className={FIELD} disabled={disabled} value={value?.settings.loudness ?? "balanced"} onChange={e => value && update({ ...value.settings, loudness: e.target.value as AudioSettings["loudness"] })}>
        <option value="off">Off</option><option value="balanced">Balanced</option><option value="loud">Loud</option>
      </select>
    </SettingRow>
    <SettingRow label="Output device" htmlFor="audio-device" hint="Where sound comes out: speakers, headphones, a Bluetooth or USB device. Applies at once and is remembered. A device that is not connected falls back to Automatic.">
      <select id="audio-device" className={FIELD} disabled={disabled || !devices} value={devices?.selected ?? "auto"} onChange={e => void chooseDevice(e.target.value)}>
        {(devices?.devices.length ? devices.devices : [{ id: "auto", label: "Automatic (system default)" }]).map(d => <option key={d.id} value={d.id}>{d.label}</option>)}
      </select>
    </SettingRow>
    {deviceError && <NoteRow tone="bad" role="alert">Could not change the output device. Try again, or pick Automatic.</NoteRow>}
    <SettingRow label="Sleep timer" htmlFor="sleep-timer" hint={endsAt ? `Pauses at ${new Date(endsAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}. Closing the app cancels it.` : "Pauses playback once after the chosen time."}>
      <select id="sleep-timer" className={FIELD} value="" onChange={e => setSleepTimer(Number(e.target.value) || null)}>
        <option value="">{endsAt ? "Running…" : "Off"}</option>{[15, 30, 45, 60, 90].map(m => <option key={m} value={m}>{m} minutes</option>)}
      </select>
      {endsAt && <Button onClick={() => setSleepTimer(null)}>Cancel</Button>}
    </SettingRow>
    <SettingRow label="Negotiated audio output" hint={OUTPUT_HINT[platform]}>
      <span>{output?.format ? `${output.driver ?? "Audio"} · ${output.format} · ${output.sampleRate ?? "?"} Hz` : "No active output"}</span>
      <Button disabled={busy} onClick={refresh}>Refresh</Button>
    </SettingRow>
    <NoteRow tone="off">Loudness changes apply right away. Other settings apply when you load the next track. Saved on this device. Compressed sources remain compressed quality.</NoteRow>
    {error && <NoteRow tone="bad" role="alert">Audio settings unavailable. Try Refresh.</NoteRow>}
  </Inset>
  {value && <AudioEffectsPanel value={value.settings.effects} disabled={busy || !value.available} onChange={(effects) => void update({ ...value.settings, effects })} />}
  </>;
}
