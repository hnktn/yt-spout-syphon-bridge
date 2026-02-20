import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

interface PreviewFramePayload {
  data: string; // base64 エンコードされた RGBA ピクセルデータ
}

const PREVIEW_WIDTH = 1280;
const PREVIEW_HEIGHT = 720;

export default function PreviewCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Tauri Event リスナーを登録
    const unlisten = listen<PreviewFramePayload>("preview-frame", (event) => {
      const { data } = event.payload;

      // base64 → Uint8Array に復号
      const binary = atob(data);
      const len = binary.length;
      const pixels = new Uint8ClampedArray(len);
      for (let i = 0; i < len; i++) {
        pixels[i] = binary.charCodeAt(i);
      }

      // ImageData を作成して Canvas に描画
      const imageData = new ImageData(pixels, PREVIEW_WIDTH, PREVIEW_HEIGHT);
      ctx.putImageData(imageData, 0, 0);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <div className="flex flex-col gap-2">
      <label className="text-xs text-gray-400 font-medium uppercase tracking-wide">
        プレビュー
      </label>
      <canvas
        ref={canvasRef}
        width={PREVIEW_WIDTH}
        height={PREVIEW_HEIGHT}
        className="w-full aspect-video bg-black rounded border border-gray-700"
      />
    </div>
  );
}
