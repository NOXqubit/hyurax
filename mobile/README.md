# Hyurax: carteira para o celular

Expo SDK 57, React Native, TypeScript. Tudo sobre o app (como funciona,
segurança, a API do nó, como instalar e desenvolver) está em
[docs/APP-CELULAR.md](../docs/APP-CELULAR.md).

- `src/nucleo/`: endereço, transação, arquivo de carteira e cliente do nó,
  conferidos contra `../vectors` (`npm run testar`).
- `src/estado/`: onde a carteira fica no aparelho.
- `src/app/`: as telas (Expo Router).
- `src/vendor/qrcode.js`: gerador de QR (Kazuhiko Arase, MIT).
- `android/` e `ios/` não existem no repositório: o Expo gera a partir do
  `app.json` (`npx expo prebuild`).

Rede de TESTE: o HYX não tem valor.
