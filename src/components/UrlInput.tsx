interface UrlInputProps {
  url: string;
  onChange: (url: string) => void;
  onSubmit: () => void;
  disabled?: boolean;
}

export default function UrlInput({ url, onChange, onSubmit, disabled }: UrlInputProps) {
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
    </div>
  );
}
