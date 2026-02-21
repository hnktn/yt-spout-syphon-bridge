import { Play, Loader2 } from "lucide-react";

interface UrlInputProps {
  url: string;
  onChange: (url: string) => void;
  onSubmit: () => void;
  disabled?: boolean;
  isLoading?: boolean;
}

export default function UrlInput({ url, onChange, onSubmit, disabled, isLoading }: UrlInputProps) {
  return (
    <div className="flex gap-1">
      <input
        type="url"
        value={url}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && onSubmit()}
        placeholder="YOUTUBE URL"
        disabled={disabled}
        className="flex-1 bg-surface-2 border border-surface-border-2 rounded-sm px-2 py-1.5 text-xs
                   text-text-primary placeholder-text-muted outline-none font-mono
                   focus:bg-surface-3 focus:border-surface-border-3 transition-colors
                   disabled:opacity-30 disabled:cursor-not-allowed"
      />
      <button
        onClick={onSubmit}
        disabled={disabled || isLoading}
        className="w-8 h-8 rounded-sm bg-surface-2 border border-surface-border-2 hover:bg-surface-3 hover:border-surface-border-3 transition-colors
                   disabled:opacity-30 disabled:cursor-not-allowed
                   flex items-center justify-center"
        title={isLoading ? "読み込み中..." : "再生"}
      >
        {isLoading ? (
          <Loader2 className="w-4 h-4 animate-spin text-text-secondary" />
        ) : (
          <Play className="w-4 h-4 text-text-primary" fill="currentColor" />
        )}
      </button>
    </div>
  );
}
