# NoEcho installer

The installer includes the optional VB-CABLE setup files in `installer/payload`.

NoEcho uses the normal Pack45 cable when a shared channel is needed. Cable A and Cable B are left alone, so they can continue to be used by MicVST and Mic Mix.

## For the person using NoEcho

1. Run the NoEcho installer.
2. If NoEcho asks for preparation, press **Prepare shared channel** once.
3. Select Discord or another app.
4. Press **Hide from remote**.
5. When finished, press **Restore normal audio**.

The person can change the interface language in **Options**. It is saved automatically.

## Generate the installer

```powershell
npm run installer
```

The finished installer is placed in `dist-installer`.
