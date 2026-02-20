import { PlayerStatus } from "../hooks/usePlayer";

interface TransportControlsProps {
  status: PlayerStatus;
  onPlay: () => void;
  onStop: () => void;
  onPause: () => void;
  disabled?: boolean;
}

export default function TransportControls({
  status,
  onPlay,
  onStop,
  onPause,
  disabled,
}: TransportControlsProps) {
  const isPlaying = status.status === "playing";
  const isPaused = status.status === "paused";
  const isLoading = status.status === "loading";
  const isActive = isPlaying || isPaused || isLoading;

  return (
    <div className="flex gap-2">
      {/* 再生 / 一時停止ボタン */}
      {isActive ? (
        <button
          onClick={onPause}
          disabled={isLoading}
          className="flex-1 py-2 rounded bg-[#2e2e2e] hover:bg-[#3a3a3a] transition-colors
                     text-sm font-medium disabled:opacity-50"
        >
          {isPaused ? "▶ 再開" : "⏸ 一時停止"}
        </button>
      ) : (
        <button
          onClick={onPlay}
          disabled={disabled || isLoading}
          className="flex-1 py-2 rounded bg-red-700 hover:bg-red-600 transition-colors
                     text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
        >
          ▶ 再生
        </button>
      )}

      {/* 停止ボタン */}
      {isActive && (
        <button
          onClick={onStop}
          className="px-4 py-2 rounded bg-[#2e2e2e] hover:bg-[#3a3a3a] transition-colors text-sm"
        >
          ■ 停止
        </button>
      )}
    </div>
  );
}
