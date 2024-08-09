# media2matome4rs
whisperでmp4を文字起こし、重要項目をリスト、項目に対して要約する。

## const
ANTHROPIC_API_KEY

## usage
- ffmpeg
- WisperAI
- ClaudeAI

## options
- cargo run main.rs/programname -i ***.mp4
  

## output/result
実行したディレクトリに/tempを作り、音声ファイル・字幕ファイル・文字起こしファイル・リザルト.mdが出力されます。