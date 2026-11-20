# Publicar versiones y actualizaciones

Un commit o un push a `main` **no** publican nada: solo compilan y prueban. Una versión se
publica únicamente al subir una etiqueta `vX.Y.Z` y pasa por `.github/workflows/release.yml`.

## Canales

| Canal | Etiqueta | Manifiesto que lee la app |
|---|---|---|
| **Estable** (predeterminado) | `v1.2.3` | `releases/latest/download/latest.json` (GitHub nunca marca una prerelease como "latest") |
| **Beta** | `v1.2.3-beta.1`, `v1.2.3-rc.1` | `releases/download/updater-beta/beta.json` (se actualiza con cada versión, beta o estable) |

La app solo instala paquetes **firmados** con la clave del proyecto: el plugin oficial de Tauri
verifica la firma minisign con la clave pública incluida en la compilación antes de instalar.
Si la compilación no tiene clave pública, las actualizaciones quedan desactivadas.

## Configuración inicial (una sola vez, la hace el propietario)

La clave privada **nunca** se guarda en el repositorio ni en el código distribuido.

1. En tu PC, desde `ui/`:
   ```powershell
   npx tauri signer generate -w "$env:USERPROFILE\.bouteca\updater.key"
   ```
   Elegí una contraseña. Guardá el archivo `.key` y la contraseña en un lugar seguro (si se
   pierden, las instalaciones existentes no podrán recibir actualizaciones firmadas con otra clave).
2. En GitHub → *Settings → Secrets and variables → Actions*:
   - **Secret** `TAURI_SIGNING_PRIVATE_KEY`: el contenido del archivo `updater.key`.
   - **Secret** `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: la contraseña.
   - **Variable** `TAURI_UPDATER_PUBKEY`: el contenido de `updater.key.pub` (es pública).

Estos secretos solo sirven para firmar en GitHub Actions; no tienen relación con las claves de
API que cada usuario carga en su app instalada.

## Publicar

1. Cambiar la versión en `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` y `ui/package.json`.
2. Actualizar `PROGRESS.md` / notas.
3. `git tag v0.3.0 && git push origin v0.3.0` (o `v0.3.0-beta.1` para beta).

El flujo verifica que la etiqueta coincida con la versión, corre las pruebas, compila el
instalador firmado, genera `latest.json` (versión, notas, fecha, firma y URL) y publica el
instalador, su `.sig`, `SHA256SUMS.txt` y el manifiesto. Sin la firma configurada se detiene
antes de compilar.

## Qué ve el usuario

- Unos segundos después de abrir (si "Buscar al iniciar" está activo) la app consulta el canal.
- Si hay versión nueva y "Descargar en segundo plano" está activo, se descarga y se verifica;
  aparece el logo con un anillo de progreso real.
- **Instalar siempre requiere confirmación** ("Reiniciar y actualizar"). El instalador NSIS
  corre en modo pasivo por usuario (sin administrador) y la app se reinicia.
- Cualquier error (sin red, firma inválida, descarga cortada) deja la versión actual intacta y
  se informa; se puede reintentar con "Buscar ahora".
