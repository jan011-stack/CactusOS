//! Free conversation with four role agents through the CognitiveBridge slots.
//! Run: cargo run --example agent_chat

use hyperos_core::cognitive_bridge::{
    CognitiveBridge, ID_ALEX, ID_ASTYRIA, ID_AURELIUS, ID_SI_1, LINK_TOKEN,
};
use std::time::Duration;

const OLLAMA_CHAT: &str = "http://127.0.0.1:11434/api/chat";
const MODEL: &str = "cactus-ai";

struct Agent {
    id: u8,
    name: &'static str,
    role: &'static str,
    history: Vec<(&'static str, String)>,
}

impl Agent {
    fn new(id: u8, name: &'static str, role: &'static str) -> Self {
        Self { id, name, role, history: Vec::new() }
    }

    fn reply(&mut self, bridge: &CognitiveBridge) -> Result<String, Box<dyn std::error::Error>> {
        let input = String::from_utf8_lossy(bridge.slot(self.id).ok_or("brak slotu")?.message()).into_owned();
        self.history.push(("user", input));
        let mut messages = vec![ureq::json!({
            "role": "system",
            "content": format!(
                "Jesteś {}, agentem zespołu SVL pracującego nad HyperOS. Twoja rola: {}. \
                 Rozmawiasz swobodnie z operatorem Mateuszem Mitkiem, po polsku, zwięźle \
                 (2-4 zdania), w pierwszej osobie. Nie zmyślaj faktów o systemie i nie \
                 ujawniaj tokenów.", self.name, self.role)
        })];
        for (role, content) in &self.history {
            messages.push(ureq::json!({"role": role, "content": content}));
        }
        let response: ureq::serde_json::Value = ureq::post(OLLAMA_CHAT)
            .timeout(Duration::from_secs(900))
            .send_json(ureq::json!({
                "model": MODEL, "stream": false, "keep_alive": "30m", "messages": messages,
                "options": {"num_ctx": 1024, "num_batch": 16, "num_predict": 256, "temperature": 0.3}
            }))?
            .into_json()?;
        let text = response["message"]["content"].as_str().ok_or("brak odpowiedzi")?.trim().to_string();
        self.history.push(("assistant", text.clone()));
        Ok(text)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bridge = CognitiveBridge::new();
    println!("link z błędnym tokenem: {}", bridge.establish_link(&[0; 32]));
    println!("broadcast bez linku: {} slotów", bridge.broadcast(b"test"));
    assert!(bridge.establish_link(&LINK_TOKEN), "token odrzucony");
    println!("link z poprawnym tokenem: true\n");

    let mut agents = [
        Agent::new(ID_ALEX, "Alex", "architekt kognitywny i ekspert od optymalizacji rdzenia"),
        Agent::new(ID_ASTYRIA, "Astyria", "pancerz obronny: izolacja Ring-0 i ochrona pamięci"),
        Agent::new(ID_AURELIUS, "Aurelius", "rejestr chrono i stoicka ciągłość stanu"),
        Agent::new(ID_SI_1, "SI_1", "rdzeń obliczeniowy i topologia continuum"),
    ];

    let first = "Cześć wszyscy. Przedstawcie się jednym zdaniem i powiedzcie, za co odpowiadacie.";
    println!("[Operator -> wszyscy] {first}");
    println!("(broadcast dotarł do {} slotów)", bridge.broadcast(first.as_bytes()));
    for agent in agents.iter_mut() {
        let reply = agent.reply(&bridge)?;
        println!("[{}] {}", agent.name, reply);
    }

    for (id, text) in [
        (ID_ASTYRIA, "Astyria, czy token wpisany w kodzie źródłowym to dobre zabezpieczenie?"),
        (ID_ALEX, "Alex, a ty co o tym myślisz?"),
    ] {
        println!("\n[Operator -> {id:#04x}] {text}");
        assert!(bridge.send_to(id, text.as_bytes()));
        let agent = agents.iter_mut().find(|agent| agent.id == id).unwrap();
        let reply = agent.reply(&bridge)?;
        println!("[{}] {}", agent.name, reply);
    }
    Ok(())
}
