interface UrlInputProps {
  url: string;
  onChange: (url: string) => void;
  onSubmit: () => void;
  disabled?: boolean;
  isLoading?: boolean;
}

export default function UrlInput({ url, onChange, onSubmit, disabled, isLoading }: UrlInputProps) {
  return (
    <div className="flex gap-2">
      <input
        type="url"
        value={url}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && onSubmit()}
        placeholder="YouTube URL を入力..."
        disabled={disabled}
        className="flex-1 bg-[#2e2e2e] border border-[#444] rounded px-3 py-2 text-sm
                   text-white placeholder-gray-500 outline-none
                   focus:border-red-500 transition-colors
                   disabled:opacity-50 disabled:cursor-not-allowed"
      />
      <button
        onClick={onSubmit}
        disabled={disabled || isLoading}
        className="w-10 h-10 rounded bg-red-700 hover:bg-red-600 transition-colors
                   disabled:opacity-50 disabled:cursor-not-allowed
                   flex items-center justify-center text-lg"
        title={isLoading ? "読み込み中..." : "再生"}
      >
        {isLoading ? (
          <div className="animate-spin">⟳</div>
        ) : (
          "▶"
        )}
      </button>
    </div>
  );
}
