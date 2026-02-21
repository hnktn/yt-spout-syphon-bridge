import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface PlayerControlsProps {
  isPlaying: boolean;
  onPause: () => void;
  onStop: () => void;
}

export default function PlayerControls({ isPlaying, onPause, onStop }: PlayerControlsProps) {
  const [loop, setLoop] = useState(false);
  const [speed, setSpeed] = useState(1.0);
  const [timePos, setTimePos] = useState(0);
  const [duration, setDuration] = useState(0);
  const [initialized, setInitialized] = useState(false);
  const [mediaTitle, setMediaTitle] = useState("");

  // 再生位置とタイトルを定期的に更新
  useEffect(() => {
    if (!isPlaying) return;

    const interval = setInterval(async () => {
      try {
        const pos = await invoke<number>("get_time_pos");
        const dur = await invoke<number>("get_duration");
        setTimePos(pos);
        setDuration(dur);
      } catch (err) {
        // エラーは無視（mpv が停止している場合など）
        console.debug("Failed to get time position:", err);
      }
    }, 500); // 1000ms → 500ms (より滑らかなUI更新)

    return () => clearInterval(interval);
  }, [isPlaying]);

  // 初期状態を取得（isPlaying が true になった時のみ）
  useEffect(() => {
    if (!isPlaying) {
      setInitialized(false);
      setMediaTitle("");
      return;
    }

    if (initialized) return;

    // 初回のみ実行
    const initializeState = async () => {
      try {
        const [currentLoop, currentSpeed, title] = await Promise.all([
          invoke<boolean>("get_loop"),
          invoke<number>("get_speed"),
          invoke<string>("get_media_title"),
        ]);
        console.log("Initialized - loop:", currentLoop, "speed:", currentSpeed, "title:", title);
        setLoop(currentLoop);
        setSpeed(currentSpeed);
        setMediaTitle(title);
        setInitialized(true);
      } catch (err) {
        console.error("Failed to initialize player controls:", err);
        setSpeed(1.0);
        setLoop(false);
        setMediaTitle("");
      }
    };

    initializeState();
  }, [isPlaying, initialized]);

  const handleLoopToggle = async () => {
    try {
      const newLoop = !loop;
      await invoke("set_loop", { enabled: newLoop });
      setLoop(newLoop);
    } catch (err) {
      console.error("Loop toggle failed:", err);
    }
  };

  const handleSpeedChange = async (newSpeed: number) => {
    try {
      console.log("Setting speed to:", newSpeed);

      // 楽観的更新
      setSpeed(newSpeed);

      await invoke("set_speed", { speed: newSpeed });

      // 設定後の値を確認
      setTimeout(async () => {
        try {
          const actualSpeed = await invoke<number>("get_speed");
          console.log("Actual speed after change:", actualSpeed);
          if (Math.abs(actualSpeed - newSpeed) > 0.01) {
            // 期待値と異なる場合のみ更新
            setSpeed(actualSpeed);
          }
        } catch (e) {
          console.error("Failed to get speed:", e);
        }
      }, 100);
    } catch (err) {
      console.error("Speed change failed:", err);
      // エラー時は元に戻す
      try {
        const currentSpeed = await invoke<number>("get_speed");
        setSpeed(currentSpeed);
      } catch (e) {
        setSpeed(1.0);
      }
    }
  };

  const handleSeek = async (seconds: number) => {
    try {
      await invoke("seek", { seconds });
    } catch (err) {
      console.error("Seek failed:", err);
    }
  };

  const formatTime = (seconds: number) => {
    if (!isFinite(seconds) || seconds < 0) return "00:00";
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
  };

  if (!isPlaying) return null;

  return (
    <div className="flex flex-col gap-2 p-2 bg-gray-800 rounded border border-gray-700">
      {/* 動画タイトル */}
      {mediaTitle && (
        <div className="text-sm font-medium text-white truncate" title={mediaTitle}>
          {mediaTitle}
        </div>
      )}

      {/* 再生時間表示とシークバー */}
      <div className="flex flex-col gap-1">
        <div className="flex justify-between text-xs text-gray-400">
          <span>{formatTime(timePos)}</span>
          <span>{formatTime(duration)}</span>
        </div>
        <input
          type="range"
          min="0"
          max={duration || 100}
          value={timePos}
          onChange={(e) => handleSeek(parseFloat(e.target.value))}
          className="w-full h-1 bg-gray-700 rounded-lg appearance-none cursor-pointer accent-blue-500"
          disabled={!duration}
        />
      </div>

      {/* コントロールボタン */}
      <div className="flex items-center justify-between gap-2 flex-wrap">
        {/* 再生制御ボタン */}
        <div className="flex gap-1">
          <button
            onClick={onPause}
            className="px-2 py-1 text-xs bg-gray-700 text-gray-300 rounded hover:bg-gray-600 transition-colors"
            title="一時停止 / 再開"
          >
            ⏯
          </button>
          <button
            onClick={onStop}
            className="px-2 py-1 text-xs bg-gray-700 text-gray-300 rounded hover:bg-gray-600 transition-colors"
            title="停止"
          >
            ⏹
          </button>
        </div>

        {/* ループトグル */}
        <button
          onClick={handleLoopToggle}
          className={`px-2 py-1 text-xs rounded transition-colors ${
            loop
              ? "bg-blue-600 text-white hover:bg-blue-700"
              : "bg-gray-700 text-gray-300 hover:bg-gray-600"
          }`}
          title={loop ? "ループ再生中" : "ループ再生オフ"}
        >
          🔁 {loop ? "ON" : "OFF"}
        </button>

        {/* 再生速度 */}
        <div className="flex items-center gap-2">
          <label className="text-xs text-gray-400">速度:</label>
          <select
            value={speed}
            onChange={(e) => handleSpeedChange(parseFloat(e.target.value))}
            className={`px-2 py-1 text-xs rounded border focus:outline-none focus:border-blue-500 ${
              Math.abs(speed - 1.0) > 0.01
                ? "bg-blue-600 text-white border-blue-500"
                : "bg-gray-700 text-gray-200 border-gray-600"
            }`}
          >
            <option value={0.25}>0.25x</option>
            <option value={0.5}>0.5x</option>
            <option value={0.75}>0.75x</option>
            <option value={1.0}>1.0x (標準)</option>
            <option value={1.25}>1.25x</option>
            <option value={1.5}>1.5x</option>
            <option value={2.0}>2.0x</option>
            <option value={4.0}>4.0x</option>
          </select>
        </div>

        {/* シークボタン */}
        <div className="flex gap-1">
          <button
            onClick={() => handleSeek(Math.max(0, timePos - 10))}
            className="px-1.5 py-0.5 text-xs bg-gray-700 text-gray-300 rounded hover:bg-gray-600 transition-colors"
            title="10秒戻る"
          >
            -10s
          </button>
          <button
            onClick={() => handleSeek(timePos + 10)}
            className="px-1.5 py-0.5 text-xs bg-gray-700 text-gray-300 rounded hover:bg-gray-600 transition-colors"
            title="10秒進む"
          >
            +10s
          </button>
        </div>
      </div>
    </div>
  );
}
