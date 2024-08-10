// sequence
// 1. 動画または音声ファイルのパスを受け取る
// 2. 動画または音声ファイルを読み込む
// 3. 動画ならば、音声ファイルに変換する
// 4. 音声ファイルをwhisperでテキストに変換する
// 5. 文字起こししたテキストをClaudeAIに渡し、要約する
// 6. 要約したテキストを返す

use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use reqwest::Client;
use serde::{Deserialize, Serialize};

const CHANNELID: u8 = 1;

const USEMODEL: &str = "claude-3-5-sonnet-20240620";

#[tokio::main]
async fn main() {
    // 引数を取得する
    let args = get_flag().unwrap();

    let file_path = &args[2];
    println!("input file path: {:?}", file_path);
    let file_name = Path::new(file_path).file_stem().unwrap().to_str().unwrap();
    let file_extension = Path::new(file_path).extension().unwrap().to_str().unwrap();

    println!(
        "file path: {:?}, filename: {}, file exe: {}",
        file_path, file_name, file_extension
    );

    // convert_to_audio 動画ファイルを音声ファイルに変換する
    // use command ffmpeg
    // audio_file_pathは音声ファイルの絶対パス
    let audio_file_path = if file_extension == "mp4" {
        convert_to_audio(file_path).unwrap()
    } else {
        file_path.to_string()
    };

    let output = convert_to_text(&audio_file_path).unwrap();
    // read text file
    let output_string = fs::read_to_string(output).unwrap();

    println!("文字起こし本文: {:?}", output_string);
    // 一時的に保存
    // API errorを回避
    temp_write_to_file(output_string.clone()).unwrap();

    // request claude
    let result = match request_claude(&output_string).await {
        Ok(result) => result,
        Err(e) => {
            println!("Error: {:?}", e);
            return;
        }
    };
    println!("result: {:?}", result);
}

// get_flag 実行コマンドのオプションを取得する
fn get_flag() -> Result<Vec<String>, &'static str> {
    // get command line options

    let args: Vec<String> = env::args().collect();

    if args.contains(&"-i".to_string()) {
        Ok(args)
    } else {
        Err("Invalid flag")
    }
}

fn get_file_extension(file_path: &str) -> &str {
    Path::new(file_path).extension().unwrap().to_str().unwrap()
}

// convert_to_audio 動画ファイルを音声ファイルに変換する
// use command ffmpeg
fn convert_to_audio(file_path: &str) -> Result<String, std::io::Error> {
    let file_name = Path::new(file_path).file_stem().unwrap().to_str().unwrap();
    let parent_dir = env::current_dir().unwrap();
    let temp_dir = parent_dir.join("temp");

    // tempディレクトリを作成
    fs::create_dir_all(&temp_dir)?;

    let output_path = temp_dir.join(format!("{}.wav", file_name));
    let audio_file_path = output_path.to_str().unwrap();

    println!("audio file path: {:?}", audio_file_path);

    let output = Command::new("ffmpeg")
        .args([
            "-i",
            file_path,
            "-vn",
            "-acodec",
            "pcm_s16le",
            "-ar",
            "44100",
            "-ac",
            CHANNELID.to_string().as_str(),
            audio_file_path,
        ])
        .output()?;

    println!("{:?}", output);

    Ok(audio_file_path.to_string())
}

// convert_to_text 音声ファイルをテキストに変換する
// use command whisper
fn convert_to_text(audio_file_path: &str) -> Result<String, std::io::Error> {
    let file_name = Path::new(audio_file_path)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap();
    let current_dir = env::current_dir().unwrap();
    let output_path = current_dir.join("temp");

    if !output_path.exists() {
        fs::create_dir_all(&output_path)?;
    }

    // output_path to string
    let output_path = output_path.to_str().unwrap();

    let text_file_path = format!("{}/{}.txt", output_path, file_name);

    let text_file_path = Path::new(&text_file_path);

    println!("audio file path: {:?}", audio_file_path);
    println!("text file path: {:?}", text_file_path);

    let output = Command::new("whisper")
        .args([
            // whisper "$audioname" --language Japanese --word_timestamps True
            audio_file_path,
            "--language",
            "Japanese",
            "--word_timestamps",
            "True",
            "--model",
            "small",
            "--output_dir",
            output_path,
        ])
        .output()?;

    println!("success output text file path: {:?}", output);

    Ok(text_file_path.to_str().unwrap().to_string())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct RequestBody {
    model: String,
    system: Option<String>,
    max_tokens: u32,
    messages: Vec<Message>,
}

async fn request_claude(result: &str) -> Result<String, reqwest::Error> {
    // 環境変数からAPIキーを取得
    let api_key = env::var("ANTHROPIC_API_KEY").expect("Expected an API key");
    let set_model: &str = USEMODEL;

    let prompt = format!("以下の文章は音声の文字起こしであり、誤字脱字や不完全な部分が含まれています。内容を慎重に読み取り、整合性を持たせて理解してください。\n\n

1. 重要な項目をリストアップしてください。\n\n

2. リストアップした各項目について、以下の点を考慮しながら要約してください：\n
   - 対象：リスナー、参加者、参加意欲はあったが参加できなかった方々\n
   - 目的：対象の理解促進\n

3. 出力形式：\n
   マークダウン形式で、以下の構造を使用してください：\n

   ## 重要項目1\n
   - 要約文1\n
   - 要約文2\n
   ※ 補足情報（必要な場合）\n

   ## 重要項目2\n
   ...（以下同様）\n

4. 専門用語や略語がある場合は、簡単な説明を付け加えてください。\n\n

5. 要約全体の長さは、元の文章の約1/3を目安としてください。\n\n

以下に文字起こしの本文を示します：\n

```{}```", result);

    let messages = vec![Message {
        role: "user".to_string(),
        content: prompt,
    }];

    let body = RequestBody {
        model: set_model.to_string(),
        system: Some("出力は日本語かつ、理解しやすい言葉を使用し、可能であれば補足情報を付け加えてください。".to_string()),
        max_tokens: 4096,
        messages,
    };

    // println!("{}", api_key);
    // println!(
    //     "{:?}: {}",
    //     json!(body),
    //     serde_json::to_string(&body).unwrap()
    // );

    // リクエストを作成
    let client = Client::new();
    let res = match client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(res) => res,
        Err(e) => {
            println!("Error: {:?}", e);
            return Err(e);
        }
    };

    // レスポンスを取得
    let res_json: Value = match res.json().await {
        Ok(res_json) => res_json,
        Err(e) => {
            println!("Error: {:?}", e);
            return Err(e);
        }
    };

    // レスポンスを整形し、保存
    let saved_result = write_to_file(res_json);

    Ok(saved_result.unwrap())
}

fn output_dir() -> String {
    let current_dir = env::current_dir().unwrap();
    let output_path = current_dir.join("temp");
    let output_path = output_path.to_str().unwrap();
    output_path.to_string()
}

fn temp_write_to_file(res: String) -> Result<(), std::io::Error> {
    // to file .txt
    let output_path = output_dir();
    let output_file_path = format!("{}/transcription.txt", output_path);

    println!("output raw result: {:?}", res);

    // write to file
    fs::write(output_file_path, &res).unwrap();

    Ok(())
}
fn write_to_file(res: Value) -> Result<String, std::io::Error> {
    // // resの中身を取得
    // println!("------------------");
    // println!("{:?}", res);
    // println!("------------------");

    // contents[0].textを取得
    let result = res["content"]
        .as_array()
        .and_then(|content| content.first())
        .and_then(|first_item| first_item["text"].as_str())
        .map(String::from);

    // to file .txt
    let output_path = output_dir();
    let output_file_path = format!("{}/result.md", output_path);

    println!("output raw result: {:?}", res);

    // write to file
    let raw_text = result.unwrap();
    fs::write(output_file_path, &raw_text).unwrap();

    Ok(raw_text)
}

// test
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_flag() {
        let args = vec!["-i".to_string(), "test.mp4".to_string()];
        let result = get_flag();
        assert_eq!(result, Ok(args));
    }

    #[test]
    fn test_get_file_extension() {
        let file_path = "test.mp4";
        let result = get_file_extension(file_path);
        println!("{:?}", result);
        assert_eq!(result, "mp4");
    }

    #[test]
    fn test_convert_to_audio() {
        let file_name = "test";
        let file_path: &str = &format!("{}.mp4", file_name);
        let result = convert_to_audio(file_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_to_text() {
        let file_name = "test";
        let audio_file_path: &str = &format!("{}.wav", file_name);
        let result = convert_to_text(audio_file_path);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_request_claude() {
        let mut result = String::from("文字起こしのテストです。");
        // 途中経過である文字起こしのファイルが有れば、読み込み使用します
        let file_name = "./temp/transaction.txt";
        // ファイルの有無
        let file = Path::new(file_name);
        if file.exists() {
            let output = fs::read_to_string(file).unwrap();
            result = output;
        }

        let result = request_claude(&result).await;
        println!("{:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_to_file() {
        let res = json!({
            "content": [
                {
                    "text": "これはテストです。"
                }
            ]
        });
        let result = write_to_file(res);
        assert!(result.is_ok());
    }
}
