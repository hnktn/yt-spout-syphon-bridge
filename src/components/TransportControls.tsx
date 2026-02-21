import { PlayerStatus } from "../hooks/usePlayer";

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
                 text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
    >
      {isLoading ? "読み込み中..." : "▶ 再生"}
    </button>
  );
}
