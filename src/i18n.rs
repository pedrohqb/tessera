//! A deliberately small translation layer: English strings in the code, and a
//! table for Brazilian Portuguese, picked from LC_ALL / LC_MESSAGES / LANG.
//! Enough for a handful of screens without a gettext toolchain.

use std::sync::OnceLock;

fn portuguese() -> bool {
    static PT: OnceLock<bool> = OnceLock::new();
    *PT.get_or_init(|| {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
            if let Ok(v) = std::env::var(var) {
                if v.is_empty() {
                    continue;
                }
                return v.starts_with("pt");
            }
        }
        false
    })
}

/// Translate a UI string.
pub fn t(s: &'static str) -> &'static str {
    if !portuguese() {
        return s;
    }
    PT_BR
        .iter()
        .find(|(en, _)| *en == s)
        .map(|(_, pt)| *pt)
        .unwrap_or(s)
}

const PT_BR: &[(&str, &str)] = &[
    // Window and menu
    ("Tessera", "Tessera"),
    ("Token hub", "Central de tokens"),
    ("Start automatically at login", "Iniciar automaticamente no login"),
    ("About Tessera", "Sobre o Tessera"),
    ("Quit", "Sair"),
    ("Refresh", "Atualizar"),
    // Service
    ("Service", "Serviço"),
    ("Token service", "Serviço de tokens"),
    ("Running", "Em execução"),
    ("Stopped", "Parado"),
    ("Start", "Iniciar"),
    ("Restart", "Reiniciar"),
    ("Smart card service (pcscd)", "Serviço de cartões (pcscd)"),
    ("Reachable", "Acessível"),
    ("Not reachable: is pcscd installed and running on the system?",
     "Inacessível: o pcscd está instalado e em execução no sistema?"),
    ("Another program already answers on this socket (is Tessera already running, for example as `tessera serve`?).",
     "Outro programa já responde neste soquete (o Tessera já está rodando, por exemplo como `tessera serve`?)."),
    ("The server exited", "O servidor foi encerrado"),
    // Readers / tokens
    ("Readers", "Leitoras"),
    ("No reader found", "Nenhuma leitora encontrada"),
    ("Connect a token or card reader.", "Conecte um token ou uma leitora de cartão."),
    ("Card present", "Cartão presente"),
    ("Empty", "Vazia"),
    ("Tokens", "Tokens"),
    ("No token found", "Nenhum token encontrado"),
    ("Insert a token, or install the driver for it below.",
     "Insira um token ou instale o driver dele abaixo."),
    ("certificate", "certificado"),
    ("certificates", "certificados"),
    ("Issued by", "Emitido por"),
    ("Valid until", "Válido até"),
    ("Expired on", "Expirou em"),
    ("No certificates visible before login", "Nenhum certificado visível antes do login"),
    // Drivers
    ("Drivers", "Drivers"),
    ("No driver installed", "Nenhum driver instalado"),
    ("Driver files missing", "Arquivos do driver ausentes"),
    // Apps
    ("Apps", "Aplicativos"),
    ("Apps that use the tokens through Tessera. Each needs the two steps once.",
     "Aplicativos que usam os tokens por meio do Tessera. Cada um precisa dos dois passos, uma vez."),
    ("Copy the command that lets this app reach Tessera",
     "Copiar o comando que permite a este aplicativo acessar o Tessera"),
    ("Command copied", "Comando copiado"),
    ("Set up", "Configurar"),
    ("Could not set up", "Não foi possível configurar"),
    ("Not set up", "Não configurado"),
    ("driver(s)", "driver(s)"),
    ("Use this driver", "Usar este driver"),
    ("System connection (the system's p11-kit server)", "Conexão com o sistema (servidor p11-kit do sistema)"),
    ("Could not write the host module files", "Não foi possível gravar os arquivos de módulo do sistema"),
    ("A driver could not be read", "Não foi possível ler um driver"),
    ("Works once the system connection is on (no setup needed)", "Funciona com a conexão com o sistema ativa (sem configuração)"),
    ("2. Set up the app", "2. Configure o aplicativo"),
    ("Or by hand in Firefox: Settings → Privacy & Security → Security Devices → Load, module file:",
     "Ou à mão no Firefox: Configurações → Privacidade e Segurança → Dispositivos de segurança → Carregar, arquivo do módulo:"),
    ("Close Firefox first, then try again", "Feche o Firefox e tente de novo"),
    // Supervision, tray and set-up
    ("Serving", "Servindo"),
    ("published", "publicado(s)"),
    ("token(s)", "token(s)"),
    ("On", "Ativo"),
    ("Off: the apps cannot see the tokens. Run once in a terminal, then quit and reopen Tessera:",
     "Desligado: os aplicativos não veem os tokens. Rode uma vez no terminal, depois saia e abra o Tessera de novo:"),
    ("token(s) of Tessera seen by the apps", "token(s) do Tessera vistos pelos aplicativos"),
    ("driver(s) published; no token to check yet", "driver(s) publicado(s); nenhum token para conferir ainda"),
    ("The apps do not see Tessera's tokens: the system's p11-kit server is off, or was turned on after Tessera started. Run once in a terminal, then quit and reopen Tessera:",
     "Os aplicativos não veem os tokens do Tessera: o servidor p11-kit do sistema está desligado, ou foi ligado depois de o Tessera abrir. Rode uma vez no terminal, depois saia e abra o Tessera de novo:"),
    ("Checking…", "Verificando…"),
    ("Ready", "Pronto"),
    ("Step 1 missing", "Falta o passo 1"),
    ("Step 2 missing", "Falta o passo 2"),
    ("Firefox is set up", "Firefox configurado"),
    ("No Firefox profile found: open Firefox once, then try again",
     "Nenhum perfil do Firefox encontrado: abra o Firefox uma vez e tente de novo"),
    ("1. The app may reach the system's p11-kit server ✓",
     "1. O aplicativo já pode acessar o servidor p11-kit do sistema ✓"),
    ("1. Let the app reach the system's p11-kit server (run once in a terminal, then restart the app)",
     "1. Permita que o aplicativo acesse o servidor p11-kit do sistema (rode uma vez no terminal e reabra o aplicativo)"),
    ("Close Firefox first: adds the module to every Firefox profile",
     "Feche o Firefox antes: adiciona o módulo a todos os perfis do Firefox"),
    ("Makes the certificate database the app reads when signing",
     "Cria o banco de certificados que o aplicativo lê ao assinar"),
    ("Open Tessera", "Abrir o Tessera"),
    ("Quit (stop serving the tokens)", "Sair (parar de servir os tokens)"),
    ("Switched off", "Desligado"),
    ("Published: the apps can use it", "Publicado: os aplicativos podem usar"),
    ("No answer; trying again", "Sem resposta; tentando de novo"),
    ("of", "de"),
    ("Starting…", "Iniciando…"),
    ("No answer 3 times in a row. Press Restart to try again, or switch it off.",
     "Sem resposta 3 vezes seguidas. Aperte Reiniciar para tentar de novo, ou desligue o driver."),
    ("is set up: restart it to load the module", "configurado: reinicie-o para carregar o módulo"),
    ("Nothing to add: the browser reaches the tokens through its root-certificates module once step 1 is done. The button removes the module an older Tessera added, which made it ask for the PIN twice",
     "Nada a acrescentar: depois do passo 1, o navegador chega aos tokens pelo seu módulo de certificados raiz. O botão retira o módulo que uma versão anterior do Tessera acrescentou, que fazia o navegador pedir o PIN duas vezes"),
    ("is set up: the duplicate module was removed; restart the browser", "está configurado: o módulo duplicado foi retirado; reinicie o navegador"),
    ("needs nothing more: step 1 is enough", "não precisa de mais nada: o passo 1 basta"),
    ("Could not turn on starting at login", "Não foi possível ativar o início automático"),
];
