## Summary

## Checks

- [ ] Web tests (`cd web && npm test`) if the web service changed
- [ ] Desktop build (`cd mac && npm run build`) if the tray app changed
- [ ] Android assemble (`cd android && ./gradlew :app:assembleDebug`) if the Android app changed
- [ ] Docs or changelog updated for user-visible behavior

No tokens or keystores are included. The only env file is `.env.example`.
