import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

interface PreviewFramePayload {
  width: number;
  height: number;
  data: string; // base64 エンコードされた RGB ピクセルデータ
}

export default function PreviewCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    console.log("PreviewCanvas mounted, registering event listener...");

    const canvas = canvasRef.current;
    if (!canvas) {
      console.error("Canvas ref is null");
      return;
    }

    const ctx = canvas.getContext("2d");
    if (!ctx) {
      console.error("Failed to get 2d context");
      return;
    }

    console.log("Canvas and context ready, listening for preview-frame events");

    // Tauri Event リスナーを登録
    const unlisten = listen<PreviewFramePayload>("preview-frame", (event) => {
      const { width, height, data } = event.payload;

      // Canvas サイズを動的に調整
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
      }

      // base64 → Uint8Array に復号（RGB形式）
      const binary = atob(data);
      const len = binary.length;
      const rgbPixels = new Uint8ClampedArray(len);
      for (let i = 0; i < len; i++) {
        rgbPixels[i] = binary.charCodeAt(i);
      }

      // RGB → RGBA に変換し、同時に上下反転（OpenGL と Canvas の Y 軸が逆）
      const rgbaPixels = new Uint8ClampedArray(width * height * 4);
      const rowSizeRGB = width * 3;
      const rowSizeRGBA = width * 4;

      for (let y = 0; y < height; y++) {
        // 行を反転: 上から y 行目 → 下から y 行目に配置
        const srcRowStart = y * rowSizeRGB;
        const dstRowStart = (height - 1 - y) * rowSizeRGBA;

        for (let x = 0; x < width; x++) {
          const rgbIdx = srcRowStart + x * 3;
          const rgbaIdx = dstRowStart + x * 4;

          rgbaPixels[rgbaIdx] = rgbPixels[rgbIdx];           // R
          rgbaPixels[rgbaIdx + 1] = rgbPixels[rgbIdx + 1];   // G
          rgbaPixels[rgbaIdx + 2] = rgbPixels[rgbIdx + 2];   // B
          rgbaPixels[rgbaIdx + 3] = 255;                     // A (完全不透明)
        }
      }

      // ImageData を作成して描画
      const imageData = new ImageData(rgbaPixels, width, height);
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
        width={320}
        height={180}
        className="w-full bg-black rounded border border-gray-700"
        style={{ aspectRatio: "auto" }}
      />
    </div>
  );
}
