# Контракт: новые входы рабочей области

`Workspace` получает от оболочки:

- `openKnownVault?: (id: string) => Promise<VaultOpenResult>` — открыть известный vault по
  записи хранилища устройства (`openTauriVaultById` / `openWebVaultById`). Не передан —
  список недавних и меню vault показывают записи без перехода, только «Открыть папку…».

Список берётся из уже переданного `device.snapshot().knownVaults`.
