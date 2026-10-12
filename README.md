# FastFind – Windows-এর জন্য দ্রুত ফাইল/ফোল্ডার সার্চ

Stack: Rust + Tauri v2 + React + SQLite

## আগে যা লাগবে (একবারই)
1. Node.js 18+  → https://nodejs.org
2. Rust          → https://rustup.rs
3. Visual Studio Build Tools ("Desktop development with C++" সহ)
4. WebView2 (Windows 10/11-এ সাধারণত আগে থেকেই থাকে)

## চালানো ও build
```
npm install
npm run tauri icon path\to\any-512px-logo.png   # icons তৈরি করবে (একবার)
npm run tauri dev                                 # ডেভেলপমেন্ট মোডে চালান
npm run tauri build                               # .exe + installer
```
Build হওয়া ফাইল:
- `src-tauri\target\release\fastfind.exe`
- `src-tauri\target\release\bundle\nsis\` (installer)

## ব্যবহার
- `Ctrl + Space` : উইন্ডো খুলুন/লুকান (অ্যাপ চালু থাকলে যেকোনো জায়গা থেকে)
- ↑ ↓ : বাছাই, `Enter` : খুলুন, `Ctrl+Enter` : ফোল্ডারে দেখান, `Ctrl+C` : পাথ কপি, `Esc` : লুকান
- প্রথমবার চালালে ব্যাকগ্রাউন্ডে সব ড্রাইভ index হয়; তখনই সার্চ করা যায়।
- নতুন/rename/delete হলে index নিজে থেকে আপডেট হয়।

## পরের ধাপের আইডিয়া
- System tray + Windows startup-এ auto-run
- NTFS MFT/USN Journal দিয়ে আরও দ্রুত প্রথম index (Everything-এর মতো)
- Exclude folder সেটিংস, সার্চ র‍্যাঙ্কিং উন্নত করা

## PC-তে কিছু install না করে .exe পাওয়া (GitHub Actions)
1. github.com-এ account খুলে নতুন repository বানান।
2. এই ফোল্ডারের সব ফাইল (`.github` সহ) upload করুন।
3. Actions ট্যাব → "Build FastFind (Windows)" → Run workflow।
4. ১০–২০ মিনিট পর রান শেষ হলে নিচে Artifacts থেকে `FastFind-Windows` ডাউনলোড করুন।
