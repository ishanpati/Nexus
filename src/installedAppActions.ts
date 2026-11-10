export interface LaunchOutcome { readonly launched: boolean; readonly warning: string | null }
export interface IconPixels { readonly width: number; readonly height: number; readonly rgba: readonly number[] }
type Transport = <T>(command: string, args: Record<string, unknown>) => Promise<T>;

/** No search text, paths, arguments, or frontend launch metadata cross IPC. */
export function createInstalledAppActions(invoke: Transport) {
  return {
    launch: (id: string) => invoke<LaunchOutcome>("launch_installed_app", { appId: id }),
    icon: (id: string) => invoke<IconPixels | null>("get_installed_app_icon", { appId: id }),
  };
}

/** Cache promises (including missing icons) and limit native work during scrolling. */
export function createIconLoader<T>(fetchIcon: (id: string) => Promise<T | null>) {
  const cache = new Map<string, Promise<T | null>>();
  const queue: (() => void)[] = [];
  let active = 0;
  const pump = () => {
    while (active < 2 && queue.length) { active++; queue.shift()!(); }
  };
  return (id: string): Promise<T | null> => {
    const cached = cache.get(id);
    if (cached) return cached;
    const request = new Promise<T | null>((resolve) => {
      queue.push(() => {
        void Promise.resolve().then(() => fetchIcon(id)).catch(() => null).then(resolve)
          .finally(() => { active--; pump(); });
      });
    });
    cache.set(id, request);
    pump();
    return request;
  };
}

export function validIconPixels(icon: IconPixels): boolean {
  return Number.isInteger(icon.width) && Number.isInteger(icon.height)
    && icon.width > 0 && icon.width <= 256 && icon.height > 0 && icon.height <= 256
    && icon.rgba.length === icon.width * icon.height * 4
    && icon.rgba.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255);
}
