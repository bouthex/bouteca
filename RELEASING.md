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

## Configuración inicial de la firma (una sola vez, solo con el navegador)

La clave se genera **dentro de GitHub Actions** y se guarda directamente como secretos del
repositorio; nunca aparece en registros, commits ni artefactos en claro. No hace falta instalar
nada.

1. **Crear un token temporal** (GitHub → tu foto → *Settings* → *Developer settings* →
   *Personal access tokens* → *Fine-grained tokens* → *Generate new token*):
   - Nombre: `bouteca-firma`; vencimiento: 7 días.
   - *Repository access*: *Only select repositories* → `bouthex/bouteca`.
   - *Permissions* → *Repository permissions*: **Secrets: Read and write** y
     **Variables: Read and write** (nada más).
   - Copiá el token generado.
2. **Guardarlo como secreto**: repositorio → *Settings* → *Secrets and variables* → *Actions* →
   *New repository secret* → Name `BOUTECA_ADMIN_TOKEN`, Secret: el token.
3. *(Opcional, recomendado)* otro secreto `BOUTECA_BACKUP_PASSWORD` con una frase larga que
   recuerdes: recibirás una copia de seguridad cifrada de la clave.
4. **Ejecutar**: pestaña *Actions* → "Configurar firma de actualizaciones (una sola vez)" →
   *Run workflow*. Genera la clave, prueba firmar y verificar, y guarda los secretos
   `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` y la variable
   `TAURI_UPDATER_PUBKEY`. Se niega a reemplazar una clave existente.
5. Si configuraste el paso 3: en la ejecución, descargá el artefacto
   `bouteca-firma-respaldo-cifrado` (dura 1 día) y guardalo (por ejemplo, en tu nube personal).
   Es un `.7z` cifrado con AES-256 y tu frase; solo hace falta si alguna vez se pierden los
   secretos de GitHub.
6. **Borrar el token** del paso 1 (*Fine-grained tokens* → *Delete*) y el secreto
   `BOUTECA_ADMIN_TOKEN`. Ya no se necesitan.

Todas las versiones futuras usan esta misma clave: `release.yml` verifica que la firma de cada
instalador corresponda a `TAURI_UPDATER_PUBKEY` antes de publicar.

## Publicar

1. Cambiar la versión en `src-tauri/tauri.conf.json`, `Cargo.toml` (workspace) y `ui/package.json`.
2. Actualizar `PROGRESS.md` / notas.
3. Pestaña *Actions* → "Publicar versión (firmada)" → *Run workflow* (publica la versión de
   `tauri.conf.json`), o bien subir una etiqueta: `git tag v1.0.1 && git push origin v1.0.1`
   (`v1.1.0-beta.1` para beta).

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
