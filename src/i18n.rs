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
    ("Copy the command that lets this app reach Tessera",
     "Copiar o comando que permite a este aplicativo acessar o Tessera"),
    ("Command copied", "Comando copiado"),
    ("Set up", "Configurar"),
    ("Could not set up", "Não foi possível configurar"),
    ("driver(s)", "driver(s)"),
    ("Use this driver", "Usar este driver"),
    ("System connection (the system's p11-kit server)", "Conexão com o sistema (servidor p11-kit do sistema)"),
    ("Could not write the host module files", "Não foi possível gravar os arquivos de módulo do sistema"),
    ("A driver could not be read", "Não foi possível ler um driver"),
    ("No steps: the system connection is enough", "Nenhum passo: basta a conexão com o sistema"),
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
    ("Steps 1 and 2 missing", "Faltam os passos 1 e 2"),
    ("step 2 done; restart the app", "passo 2 feito; reinicie o aplicativo"),
    ("Step 1 · access to the system's p11-kit server", "Passo 1 · acesso ao servidor p11-kit do sistema"),
    ("Done: the app can reach the system's p11-kit server", "Feito: o aplicativo chega ao servidor p11-kit do sistema"),
    ("Run once in a terminal, then restart the app:", "Rode uma vez no terminal e reinicie o aplicativo:"),
    ("Step 2 · remove the duplicate module", "Passo 2 · retirar o módulo duplicado"),
    ("Step 2 · certificate database", "Passo 2 · banco de certificados"),
    ("Done: Tessera's module is in every Firefox profile", "Feito: o módulo do Tessera está em todos os perfis do Firefox"),
    ("Done: the app has its own certificate database", "Feito: o aplicativo tem o seu banco de certificados"),
    ("An older Tessera added a module that makes the browser ask for the PIN twice. Close the browser first",
     "Uma versão anterior do Tessera acrescentou um módulo que faz o navegador pedir o PIN duas vezes. Feche o navegador antes"),
    ("Remove", "Retirar"),
    ("Apps that use the tokens through Tessera. Each needs one or two steps, once.",
     "Aplicativos que usam os tokens por meio do Tessera. Cada um precisa de um ou dois passos, uma vez."),
    ("Show an icon in the tray", "Mostrar ícone na bandeja"),
    ("Ready", "Pronto"),
    ("Step 1 missing", "Falta o passo 1"),
    ("Step 2 missing", "Falta o passo 2"),
    ("No Firefox profile found: open Firefox once, then try again",
     "Nenhum perfil do Firefox encontrado: abra o Firefox uma vez e tente de novo"),
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
    ("Could not turn on starting at login", "Não foi possível ativar o início automático"),
];
