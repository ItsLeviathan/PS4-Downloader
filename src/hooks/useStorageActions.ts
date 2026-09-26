import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../lib/api";
import { runAction, useApp } from "../stores/app";

export function useStorageActions() {
  const settings = useApp((s) => s.settings);
  const storage = useApp((s) => s.storage);
  const setSettings = useApp((s) => s.setSettings);
  const setStorage = useApp((s) => s.setStorage);

  const chooseFolder = async () => {
    if (!settings) return;
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Choose where downloads are stored",
      defaultPath: storage?.available ? settings.storageRoot : undefined,
    });
    if (typeof picked !== "string") return;
    await runAction(async () => {
      setSettings(await api.updateSettings({ ...settings, storageRoot: picked }));
      setStorage(await api.getStorageInfo());
    });
  };

  const createFolder = () => runAction(async () => setStorage(await api.createStorageFolder()));

  return { chooseFolder, createFolder };
}
