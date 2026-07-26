# Que hacer con tus VB-Cable

Tienes 3 cosas distintas:

1) VBCABLE_Driver_Pack45
   - Es el cable virtual normal (uno solo).

2) VBCABLE_A_Driver_Pack43
   - Es el Cable A.

3) VBCABLE_B_Driver_Pack43
   - Es el Cable B.

## Que usa NoEcho por defecto

NoEcho usa **Cable A** como canal compartido.

Por eso en installer/payload quedo:
- NoEchoAudioSetup.exe = instalador de Cable A
- tambien se copiaron B y el cable normal por si acaso

## Que tienes que hacer tu

Nada especial en el dia a dia.

Solo:
1. Deja esos archivos en installer/payload (ya estan)
2. Genera el instalador con: npm run installer
3. En otra PC, instala NoEcho
4. Si pide preparativo, pulsa Preparar audio (solo una vez)

## Que NO tienes que hacer

- No tienes que elegir entre A y B cada vez
- No tienes que configurar VoiceMeeter
- No tienes que explicarles a tus usuarios que es un cable virtual