# mainichikatarenshu

> 毎日型練習して！

E-ink display showing a kata for each day, along with current electricity price, using ESP-32 and written in Rust language.

Each day at 07:00 EET the e-ink screen will wake up and update the kata name from a random list.

The display will shutdown at 23:00 EET, which will record the kata either incomplete if the button has not been pressed before that.

The knowledge of the kata being confirmed or not, will be send to Google Drive, to store the information in a spreadsheet. The columns date, kata name, confirmed (boolean), time of confirmation.

```sh
cargo install espup # Get ESP tooling handler
espup install # Install ESP tools
```


## License

MIT
