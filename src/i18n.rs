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
    ("Starting…", "Iniciando…"),
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
    ("Loaded", "Carregado"),
    // Apps
    ("Apps", "Aplicativos"),
    ("Apps that use the tokens through Tessera.", "Aplicativos que usam os tokens por meio do Tessera."),
    ("Works automatically.", "Funciona automaticamente."),
    ("Allow access", "Permitir acesso"),
    ("Copy the command that lets this app reach Tessera",
     "Copiar o comando que permite a este aplicativo acessar o Tessera"),
    ("Command copied", "Comando copiado"),
    ("Set up", "Configurar"),
    ("Add to a profile…", "Adicionar a um perfil…"),
    ("Choose the Firefox profile folder", "Escolha a pasta do perfil do Firefox"),
    ("Close Firefox first. Or add it by hand: Settings → Privacy & Security → Security Devices → Load, module file:",
     "Feche o Firefox antes. Ou adicione à mão: Configurações → Privacidade e Segurança → Dispositivos de segurança → Carregar, arquivo do módulo:"),
    ("Papers is set up", "O Papers está configurado"),
    ("Firefox profile is set up", "O perfil do Firefox está configurado"),
    ("Already set up", "Já configurado"),
    ("Could not set up", "Não foi possível configurar"),
    ("Start the token service first", "Inicie o serviço de tokens antes"),
    ("Not set up", "Não configurado"),
    ("driver(s)", "driver(s)"),
    ("System connection (the system's p11-kit server)", "Conexão com o sistema (servidor p11-kit do sistema)"),
    ("module file(s) in", "arquivo(s) de módulo em"),
    ("run once in a terminal:", "rode uma vez no terminal:"),
    ("Could not write the host module files", "Não foi possível gravar os arquivos de módulo do sistema"),
    ("A driver could not be read", "Não foi possível ler um driver"),
    ("Works once the system connection is on (no setup needed)", "Funciona com a conexão com o sistema ativa (sem configuração)"),
    ("Configured", "Configurado"),
    ("1. Let the app reach the system's p11-kit server (run once in a terminal)", "1. Permita que o aplicativo acesse o servidor p11-kit do sistema (rode uma vez no terminal)"),
    ("2. Set up the app", "2. Configure o aplicativo"),
    ("Close Firefox, then choose its profile folder", "Feche o Firefox e escolha a pasta do perfil"),
    ("Papers will find the token's certificates when signing", "O Papers passará a encontrar os certificados do token ao assinar"),
    ("Or by hand in Firefox: Settings → Privacy & Security → Security Devices → Load, module file:",
     "Ou à mão no Firefox: Configurações → Privacidade e Segurança → Dispositivos de segurança → Carregar, arquivo do módulo:"),
    ("Load the module by hand in Firefox (see below)", "Carregue o módulo à mão no Firefox (veja abaixo)"),
    ("Close Firefox first, then try again", "Feche o Firefox e tente de novo"),
    ("Waiting for its FlatPark package to support Tessera", "Aguardando o pacote do FlatPark passar a usar o Tessera"),
    ("Done", "Pronto"),
];
