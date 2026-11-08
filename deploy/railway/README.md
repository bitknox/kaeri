# Run a plugin store on Railway

[![Deploy on Railway](https://railway.com/button.svg)](https://railway.com/deploy/TEMPLATE_CODE)

The button deploys the store's published image with a volume for its files. Railway bills by
usage, and its trial and free plans cap a volume at 0.5 GB.

After the deploy:

1. Open the service's Variables tab and copy `KAERI_STORE_SETUP_CODE`.
2. Open `https://<your-service>.up.railway.app/admin/` and enter the code. The page answers with
   an owner invite link.
3. In Kaeri, go to Sources, press Enter, choose join a store and paste the link.

The deploy log also shows a one-time owner invite, which works the same way.

To pick up new releases on their own, open the service's Settings, choose Configure Auto Updates
under Source, and pick a maintenance window. Railway redeploys when the template's image tag gets
a new image. To update by hand, open the service and choose Redeploy. While the major version is 0,
a new minor version needs the image tag changed under Settings, Source, and from 1.0 on a new major
does.

## Making the template

Railway's one-click deploys come from templates made in its dashboard, so this is done once, by
whoever publishes the button. The result is the URL that replaces `TEMPLATE_CODE` above and in
`deploy/README.md`.

1. In Railway, open your workspace's Templates page and choose New Template.
2. Add a service, choose Docker Image as its source, and enter
   `ghcr.io/bitknox/kaeri-store:0.4`. Name the service `kaeri-store`.
3. Right-click the service, choose Attach Volume, and set the mount path to `/data`.
4. In the service's Variables, add:

   ```
   KAERI_STORE_SETUP_CODE=${{secret(32, "abcdef0123456789")}}
   KAERI_STORE_TRUST_PROXY=true
   PORT=7878
   RAILWAY_RUN_UID=0
   ```

   Optionally add `KAERI_STORE_NAME`, described as "The store's name, as members see it".

5. In Settings, under Networking, turn on public networking over HTTP and generate a domain for
   port 7878.
6. In Settings, under Deploy, set the healthcheck path to `/healthz`.
7. Choose Create Template, then copy the template's URL from the Templates page.

What the variables are for:

- `secret(32, "abcdef0123456789")` gives every deploy its own 32-character hex setup code.
- `KAERI_STORE_TRUST_PROXY` tells the store that Railway's proxy is the only way in. The store
  also recognises Railway by itself; this keeps that from depending on which variables Railway sets.
- `PORT` fixes the port the store listens on, so the domain can point at it. Without it, Railway
  picks a port when the service starts.
- `RAILWAY_RUN_UID=0` runs the store as root. Railway mounts volumes owned by root, and documents
  this variable for images that otherwise run as a non-root user, as this one does.

`KAERI_STORE_URL` is not needed: the store reads `RAILWAY_PUBLIC_DOMAIN` itself, and Railway
puts a custom domain there once you add one. To set it anyway, use
`KAERI_STORE_URL=https://${{RAILWAY_PUBLIC_DOMAIN}}`, and only once the service has a domain:
`https://` with nothing after it stops the store from starting.
