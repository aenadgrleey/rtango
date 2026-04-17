---
name: deploy
description: Deploy the application to a staging or production environment.
allowed-tools: read shell edit
---

# Deploy

Run `./scripts/deploy.sh <env>` to ship a release.

Steps:
1. Confirm the target environment with the user.
2. Run the deploy script and stream logs.
3. Verify the health check endpoint returns 200.
