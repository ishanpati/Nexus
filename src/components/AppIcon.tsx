import { useEffect, useState } from "react";
import { createIconLoader, validIconPixels } from "../installedAppActions";
import { installedAppActions } from "../services/installedAppService";

export function makeIconLoader() {
  return createIconLoader(async (id) => {
    const icon = await installedAppActions.icon(id);
    if (!icon || !validIconPixels(icon)) return null;
    const canvas = document.createElement("canvas");
    canvas.width = icon.width;
    canvas.height = icon.height;
    const context = canvas.getContext("2d");
    if (!context) return null;
    context.putImageData(new ImageData(new Uint8ClampedArray(icon.rgba), icon.width, icon.height), 0, 0);
    return canvas.toDataURL("image/png");
  });
}

export function AppIcon({ appId, load }: { appId?: string; load: ReturnType<typeof makeIconLoader> }) {
  const [image, setImage] = useState<{ id: string; url: string } | null>(null);
  useEffect(() => {
    let disposed = false;
    setImage(null);
    if (appId) void load(appId).then((url) => { if (!disposed && url) setImage({ id: appId, url }); });
    return () => { disposed = true; };
  }, [appId, load]);
  return <span className="command-icon" aria-hidden="true">
    {image && image.id === appId ? <img src={image.url} alt="" width={28} height={28} onError={() => setImage(null)} /> :
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <rect x="5" y="3.5" width="14" height="17" rx="2" /><path d="M9 8h6M9 12h6M9 16h4" />
      </svg>}
  </span>;
}
