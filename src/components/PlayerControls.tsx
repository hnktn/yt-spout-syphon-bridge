import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Pause, Square, Repeat, ChevronLeft, ChevronRight } from "lucide-react";

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

  return (
    <div className="flex flex-col gap-2 p-2 bg-surface-1 border border-surface-border rounded-sm">
      {/* 動画タイトル（再生中のみ表示） */}
      {isPlaying && mediaTitle && (
        <div className="text-xs text-text-primary truncate uppercase tracking-wide" title={mediaTitle}>
          {mediaTitle}
        </div>
      )}

      {/* 再生時間表示とシークバー（再生中のみ有効） */}
      <div className="flex flex-col gap-1">
        <div className="flex justify-between text-xs text-text-muted font-mono">
          <span>{formatTime(timePos)}</span>
          <span>{formatTime(duration)}</span>
        </div>
        <input
          type="range"
          min="0"
          max={duration || 100}
          value={timePos}
          onChange={(e) => handleSeek(parseFloat(e.target.value))}
          className="w-full h-1 bg-surface-3 appearance-none cursor-pointer accent-accent rounded-sm disabled:opacity-30 disabled:cursor-not-allowed"
          disabled={!isPlaying || !duration}
        />
      </div>

      {/* コントロールボタン */}
      <div className="flex items-center justify-between gap-1.5 flex-wrap">
        {/* 再生制御ボタン（再生中のみ有効） */}
        <div className="flex gap-0.5">
          <button
            onClick={onPause}
            disabled={!isPlaying}
            className="w-6 h-6 flex items-center justify-center bg-surface-2 border border-surface-border-2 text-text-secondary rounded-sm hover:bg-surface-3 hover:border-surface-border-3 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            title="一時停止 / 再開"
          >
            <Pause className="w-3 h-3" />
          </button>
          <button
            onClick={onStop}
            disabled={!isPlaying}
            className="w-6 h-6 flex items-center justify-center bg-surface-2 border border-surface-border-2 text-text-secondary rounded-sm hover:bg-surface-3 hover:border-surface-border-3 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            title="停止"
          >
            <Square className="w-3 h-3" />
          </button>
        </div>

        {/* ループトグル（常に有効） */}
        <button
          onClick={handleLoopToggle}
          className={`flex items-center gap-1 px-1.5 py-0.5 text-xs rounded-sm border transition-colors ${
            loop
              ? "bg-surface-3 border-surface-border-3 text-text-primary"
              : "bg-surface-2 border-surface-border-2 text-text-muted hover:bg-surface-3 hover:border-surface-border-3"
          }`}
          title={loop ? "ループ再生中" : "ループ再生オフ"}
        >
          <Repeat className="w-3 h-3" />
          <span className="uppercase tracking-wide">{loop ? "ON" : "OFF"}</span>
        </button>

        {/* 再生速度（常に有効） */}
        <div className="flex items-center gap-1">
          <label className="text-xs text-text-muted uppercase tracking-wide">SPD</label>
          <select
            value={speed}
            onChange={(e) => handleSpeedChange(parseFloat(e.target.value))}
            className={`px-1.5 py-0.5 text-xs rounded-sm border focus:outline-none font-mono ${
              Math.abs(speed - 1.0) > 0.01
                ? "bg-surface-3 border-surface-border-3 text-accent"
                : "bg-surface-2 border-surface-border-2 text-text-secondary"
            }`}
          >
            <option value={0.25}>0.25</option>
            <option value={0.5}>0.50</option>
            <option value={0.75}>0.75</option>
            <option value={1.0}>1.00</option>
            <option value={1.25}>1.25</option>
            <option value={1.5}>1.50</option>
            <option value={2.0}>2.00</option>
            <option value={4.0}>4.00</option>
          </select>
        </div>

        {/* シークボタン（再生中のみ有効） */}
        <div className="flex gap-0.5">
          <button
            onClick={() => handleSeek(Math.max(0, timePos - 10))}
            disabled={!isPlaying}
            className="flex items-center gap-0.5 px-1 py-0.5 text-xs bg-surface-2 border border-surface-border-2 text-text-muted rounded-sm hover:bg-surface-3 hover:border-surface-border-3 transition-colors font-mono disabled:opacity-30 disabled:cursor-not-allowed"
            title="10秒戻る"
          >
            <ChevronLeft className="w-2.5 h-2.5" />
            <span>10</span>
          </button>
          <button
            onClick={() => handleSeek(timePos + 10)}
            disabled={!isPlaying}
            className="flex items-center gap-0.5 px-1 py-0.5 text-xs bg-surface-2 border border-surface-border-2 text-text-muted rounded-sm hover:bg-surface-3 hover:border-surface-border-3 transition-colors font-mono disabled:opacity-30 disabled:cursor-not-allowed"
            title="10秒進む"
          >
            <span>10</span>
            <ChevronRight className="w-2.5 h-2.5" />
          </button>
        </div>
      </div>
    </div>
  );
}
