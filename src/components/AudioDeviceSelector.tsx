import { useState, useEffect } from "react";
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

  useEffect(() => {
    getAudioDevices().then(setDevices);
  }, [getAudioDevices]);

  const handleSelect = (id: string) => {
    setSelected(id);
    onSelect(id);
  };

  const handleVolume = (v: number) => {
    setVolume(v);
    onVolumeChange(v);
  };

  return (
    <div className="flex flex-col gap-2">
      <label className="text-xs text-gray-400 font-medium uppercase tracking-wide">
        音声出力デバイス
      </label>

      <select
        value={selected}
        onChange={(e) => handleSelect(e.target.value)}
        className="bg-[#2e2e2e] border border-[#444] rounded px-3 py-2 text-sm
                   text-white outline-none focus:border-red-500 transition-colors"
      >
        <option value="">デフォルト</option>
        {devices.map((d) => (
          <option key={d.id} value={d.id}>
            {d.name}
          </option>
        ))}
      </select>

      <div className="flex items-center gap-3">
        <label className="text-xs text-gray-400 w-12">音量</label>
        <input
          type="range"
          min={0}
          max={100}
          value={volume}
          onChange={(e) => handleVolume(Number(e.target.value))}
          className="flex-1 accent-red-500"
        />
        <span className="text-xs text-gray-400 w-8 text-right">{volume}</span>
      </div>
    </div>
  );
}
