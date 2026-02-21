import { PlayerStatus } from "../hooks/usePlayer";
import { Play, Loader2 } from "lucide-react";

interface TransportControlsProps {
  status: PlayerStatus;
  onPlay: () => void;
  disabled?: boolean;
}

export default function TransportControls({
  status,
  onPlay,
  disabled,
}: TransportControlsProps) {
  const isLoading = status.status === "loading";

  return (
    <button
      onClick={onPlay}
      disabled={disabled || isLoading}
      className="w-full py-2 rounded bg-red-700 hover:bg-red-600 transition-colors
                 text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed
                 flex items-center justify-center gap-2"
    >
      {isLoading ? (
        <>
          <Loader2 className="w-4 h-4 animate-spin" />
          <span>読み込み中...</span>
        </>
      ) : (
        <>
          <Play className="w-4 h-4" fill="currentColor" />
          <span>再生</span>
        </>
      )}
    </button>
  );
}
