import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Volume2, VolumeX } from "lucide-react";
import { usePlayer, AudioDevice } from "../hooks/usePlayer";

interface AudioDeviceSelectorProps {
  onSelect: (deviceId: string) => void;
  onVolumeChange: (volume: number) => void;
}

export default function AudioDeviceSelector({ onSelect, onVolumeChange }: AudioDeviceSelectorProps) {
  const { getAudioDevices } = usePlayer();
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [selected, setSelected] = useState("");
  const [volume, setVolume] = useState(100);
  const [mute, setMute] = useState(false);

  useEffect(() => {
    getAudioDevices().then(setDevices);
  }, []);

  const handleSelect = (id: string) => {
    setSelected(id);
    onSelect(id);
  };

  const handleVolume = (v: number) => {
    setVolume(v);
    onVolumeChange(v);
  };

  const handleMuteToggle = async () => {
    try {
      const newMute = !mute;
      await invoke("set_mute", { mute: newMute });
      setMute(newMute);
    } catch (err) {
      console.error("Mute toggle failed:", err);
    }
  };

  return (
    <div className="flex flex-col gap-1.5 p-1.5 bg-surface-1 border border-surface-border rounded-sm">
      <label className="text-xs text-text-muted uppercase tracking-wide">
        AUDIO OUT
      </label>

      <select
        value={selected}
        onChange={(e) => handleSelect(e.target.value)}
        className="bg-surface-2 border border-surface-border-2 rounded-sm px-2 py-1 text-xs
                   text-text-primary outline-none focus:bg-surface-3 focus:border-surface-border-3 transition-colors font-mono"
      >
        <option value="">DEFAULT</option>
        {devices.map((d) => (
          <option key={d.id} value={d.id}>
            {d.name}
          </option>
        ))}
      </select>

      <div className="flex items-center gap-2">
        <button
          onClick={handleMuteToggle}
          className={`w-5 h-5 flex items-center justify-center rounded-sm border transition-colors ${
            mute
              ? "bg-surface-2 border-surface-border-2 text-accent hover:bg-surface-3 hover:border-surface-border-3"
              : "bg-surface-2 border-surface-border-2 text-text-secondary hover:bg-surface-3 hover:border-surface-border-3"
          }`}
          title={mute ? "ミュート解除" : "ミュート"}
        >
          {mute ? (
            <VolumeX className="w-3 h-3" />
          ) : (
            <Volume2 className="w-3 h-3" />
          )}
        </button>
        <input
          type="range"
          min={0}
          max={100}
          value={volume}
          onChange={(e) => handleVolume(Number(e.target.value))}
          className="flex-1 h-1 bg-surface-3 appearance-none cursor-pointer accent-accent rounded-sm"
        />
        <span className="text-xs text-text-secondary font-mono w-7 text-right">{volume}</span>
      </div>
    </div>
  );
}
