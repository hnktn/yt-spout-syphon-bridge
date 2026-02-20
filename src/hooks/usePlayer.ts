import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ─── 型定義 ──────────────────────────────────────────────────────────────────

export interface PlayerStatus {
  status: "idle" | "loading" | "playing" | "paused" | "error";
  url?: string;
  error?: string;
  spout_active: boolean;
  syphon_active: boolean;
}

export interface AudioDevice {
  id: string;
  name: string;
}

// ─── usePlayer フック ─────────────────────────────────────────────────────────

export function usePlayer() {
  const [status, setStatus] = useState<PlayerStatus>({
    status: "idle",
    spout_active: false,
    syphon_active: false,
  });

  // player-status イベントをリッスン
  useEffect(() => {
    const unlisten = listen<{ status: string }>("player-status", (event) => {
      setStatus((prev) => ({
        ...prev,
        status: event.payload.status as PlayerStatus["status"],
      }));
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const play = useCallback(async (url: string, quality?: string) => {
    try {
      const result = await invoke<PlayerStatus>("play", { request: { url, quality } });
      setStatus(result);
    } catch (err) {
      setStatus((prev) => ({
        ...prev,
        status: "error",
        error: String(err),
      }));
    }
  }, []);

  const stop = useCallback(async () => {
    try {
      const result = await invoke<PlayerStatus>("stop");
      setStatus(result);
    } catch (err) {
      console.error("stop failed:", err);
    }
  }, []);

  const pause = useCallback(async () => {
    try {
      const result = await invoke<PlayerStatus>("pause");
      setStatus(result);
    } catch (err) {
      console.error("pause failed:", err);
    }
  }, []);

  const setAudioDevice = useCallback(async (deviceId: string) => {
    try {
      await invoke("set_audio_device", { deviceId });
    } catch (err) {
      console.error("set_audio_device failed:", err);
    }
  }, []);

  const setVolume = useCallback(async (volume: number) => {
    try {
      await invoke("set_volume", { volume: Math.round(volume) });
    } catch (err) {
      console.error("set_volume failed:", err);
    }
  }, []);

  const getAudioDevices = useCallback(async (): Promise<AudioDevice[]> => {
    try {
      return await invoke<AudioDevice[]>("get_audio_devices");
    } catch (err) {
      console.error("get_audio_devices failed:", err);
      return [];
    }
  }, []);

  return { status, play, stop, pause, setAudioDevice, setVolume, getAudioDevices };
}
