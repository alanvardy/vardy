# Task

The fly.io production app (VAR-679) broke on deploy. Investigate why a deploy can take the app down and design a safer deployment strategy — e.g. gating deploys on CI, moving migrations out of image build into release-time steps, health-check-driven rollout, and persistent storage for the SQLite database — so that broken changes cannot reach or take down production again.
