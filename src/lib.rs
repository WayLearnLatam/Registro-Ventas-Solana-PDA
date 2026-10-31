use anchor_lang::prelude::*;

declare_id!("Qv5tfyTDS7DgD59jwDoW1TocKpcbTwAnD8TSd7dozwq");

#[program] // El codigo empieza desde aqui
pub mod registro {
    use super::*; // Importa todos los structs y enums definidos fuera del modulo

    /////////////////////////// INSTRUCCIONES ///////////////////////////
    /////////////////////////// Crear Registro ///////////////////////////
    pub fn crear_registro(context: Context<NuevoRegistro>, nombre_negocio: String) -> Result<()> {
        let owner_id = context.accounts.owner.key(); // caller wallet

        let clientes = Vec::new(); // crear un vector vacio, aqui guardaremos PDAs, no structs
        let id_counter: u16 = 0; // contador incremental, asigna un id unico a cada cliente

        context.accounts.registro.set_inner(Registro {
            owner: owner_id,
            nombre_negocio: nombre_negocio.clone(),
            clientes,
            id_counter,
        }); // crear el struct del registro, lo serializa y lo guarda en el espacio de la cuenta

        msg!("Registro: {} creado exitosamente!", nombre_negocio); // Log de verificacion

        Ok(())
    }

    /////////////////////////// Nuevo Cliente ///////////////////////////
    // "init" no puede ser condicional: Anchor crea las cuentas antes de entrar al handler.
    // Por eso en el frontend se decide: cliente nuevo -> crear_cliente, luego registrar_venta.
    pub fn crear_cliente(context: Context<NuevoCliente>, id_cliente: u16) -> Result<()> {
        require!(
            context.accounts.registro.owner == context.accounts.owner.key(),
            Errores::NoOwner
        ); // Medida de seguridad

        require!(
            id_cliente == context.accounts.registro.id_counter + 1,
            Errores::NoUser
        ); // El id de las seeds debe coincidir con el que guardara el programa

        let cliente_key = context.accounts.cliente.key(); // Se lee ANTES del &mut para no solapar prestamos

        let registro = &mut context.accounts.registro;
        registro.id_counter += 1;
        let nuevo_id = registro.id_counter;
        registro.clientes.push(cliente_key); // Agrega la PDA del cliente al vector del registro

        context.accounts.cliente.set_inner(Cliente {
            id_cliente: nuevo_id,
            compras: 0, // Nace sin compras: registrar_venta es quien incrementa
            historial: Vec::new(),
        }); // Serializa y guarda el struct en la cuenta recien creada

        msg!("Cliente creado con id: {}", nuevo_id); // Log de verificacion

        Ok(())
    }

    /////////////////////////// Registrar Venta ///////////////////////////
    // El cliente ya existe al llegar aqui: el contexto NuevaVenta lo exige inicializado
    pub fn registrar_venta(
        context: Context<NuevaVenta>,
        _id_cliente: u16,
        sku: String,
        descripcion: String,
        monto: u32,
    ) -> Result<()> {
        require!(
            context.accounts.registro.owner == context.accounts.owner.key(),
            Errores::NoOwner
        ); // Medida de seguridad

        let cliente_key = context.accounts.cliente.key(); // Se lee ANTES del &mut de cliente

        require!(
            context.accounts.registro.clientes.contains(&cliente_key),
            Errores::NoUser
        ); // Verificar que el cliente pertenece a este registro

        context.accounts.venta.set_inner(Venta {
            sku,
            descripcion,
            monto,
        }); // Serializa y guarda el struct en la cuenta de la venta

        let venta_key = context.accounts.venta.key();

        let cliente = &mut context.accounts.cliente;
        cliente.historial.push(venta_key); // El historial guarda PDAs, no structs completos
        cliente.compras += 1;

        msg!(
            "Venta registrada para el cliente con id: {}",
            cliente.id_cliente
        ); // Log de verificacion

        Ok(())
    }

    /////////////////////////// Hacer Devolucion ///////////////////////////
    // El contexto EliminarVenta lleva "close = owner": la cuenta de la venta se cierra al final
    pub fn hacer_devolucion(
        context: Context<EliminarVenta>,
        id_cliente: u16,
        sku: String,
    ) -> Result<()> {
        require!(
            context.accounts.registro.owner == context.accounts.owner.key(),
            Errores::NoOwner
        ); // Medida de seguridad

        let venta_key = context.accounts.venta.key();
        let cliente_key = context.accounts.cliente.key(); // Ambas keys ANTES del &mut del historial

        require!(
            context.accounts.registro.clientes.contains(&cliente_key),
            Errores::NoUser
        ); // Verificar que el cliente pertenece a este registro

        let historial = &mut context.accounts.cliente.historial;
        require!(!historial.is_empty(), Errores::SinCompras);
        require!(historial.contains(&venta_key), Errores::NoSKU); // Sin esto, compras -= 1 se ejecutaria igual

        historial.retain(|&k| k != venta_key); // Quita la referencia del historial

        context.accounts.cliente.compras -= 1;

        msg!(
            "Venta con sku: {}, eliminada exitosamente del cliente con id: {}",
            sku,
            id_cliente
        );

        Ok(())
    }

    /////////////////////////// Cambiar Monto ///////////////////////////
    // La cuenta de la venta llega en el contexto, no hay que recorrer ningun vector
    pub fn cambiar_monto(
        context: Context<ModificarVenta>,
        id_cliente: u16,
        sku: String,
        nuevo_monto: u32,
    ) -> Result<()> {
        require!(
            context.accounts.registro.owner == context.accounts.owner.key(),
            Errores::NoOwner
        ); // Medida de seguridad

        let cliente_key = context.accounts.cliente.key();
        let venta_key = context.accounts.venta.key();

        // Se reconstruye la cadena registro -> cliente -> venta. El require! del owner solo prueba
        // que el firmante es dueño del registro que el mismo envio, y crear uno cuesta centavos.
        require!(
            context.accounts.registro.clientes.contains(&cliente_key),
            Errores::NoUser
        );
        require!(
            context.accounts.cliente.historial.contains(&venta_key),
            Errores::NoSKU
        );

        let venta = &mut context.accounts.venta;
        let viejo_monto = venta.monto;
        venta.monto = nuevo_monto;

        msg!(
            "El monto de la transaccion con sku: {}, del usuario con id: {}, paso de: {}, a: {}",
            sku,
            id_cliente,
            viejo_monto,
            nuevo_monto
        );

        Ok(())
    }
}

/////////////////////////// Codigos de Error ///////////////////////////
#[error_code]
pub enum Errores {
    #[msg("Error, no eres el owner del registro")]
    NoOwner,
    #[msg("Error, usuario no existe")]
    NoUser,
    #[msg("Error, sku no encontrado")]
    NoSKU,
    #[msg("Error, no hay compras en el historial")]
    SinCompras,
}

/////////////////////////// CUENTAS ///////////////////////////
/////////////////////////// Registro ///////////////////////////

#[account]
#[derive(InitSpace)]
pub struct Registro {
    owner: Pubkey,

    #[max_len(30)]
    nombre_negocio: String,

    #[max_len(10)] // Guarda PDAs (32 bytes c/u), no structs completos
    clientes: Vec<Pubkey>,

    id_counter: u16,
}

/////////////////////////// Cliente ///////////////////////////

#[account]
#[derive(InitSpace, PartialEq)]
pub struct Cliente {
    id_cliente: u16,
    compras: u16,

    #[max_len(10)]
    historial: Vec<Pubkey>,
}

/////////////////////////// Venta ///////////////////////////

#[account]
#[derive(InitSpace, PartialEq)]
pub struct Venta {
    #[max_len(5)]
    sku: String,

    #[max_len(50)]
    descripcion: String,

    monto: u32,
}

/////////////////////////// CONTEXTOS ///////////////////////////
/////////////////////////// Nuevo Registro ///////////////////////////
/// Instruccion: crear_registro

#[derive(Accounts)]
#[instruction(nombre_negocio:String)]
pub struct NuevoRegistro<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = Registro::INIT_SPACE + 8,
        seeds = [b"registro", nombre_negocio.as_bytes(), owner.key().as_ref()],
        bump
    )]
    pub registro: Account<'info, Registro>,

    pub system_program: Program<'info, System>,
}

/////////////////////////// Nuevo Cliente ///////////////////////////
/// Instruccion: crear_cliente

#[derive(Accounts)]
#[instruction(id_cliente:u16)]
pub struct NuevoCliente<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub registro: Account<'info, Registro>,

    #[account(
        init,
        payer = owner,
        space = Cliente::INIT_SPACE + 8,
        // to_le_bytes() porque un u16 no tiene as_bytes(): ese metodo es de String/str
        seeds = [b"cliente", id_cliente.to_le_bytes().as_ref(), owner.key().as_ref()],
        bump
    )]
    pub cliente: Account<'info, Cliente>,

    pub system_program: Program<'info, System>,
}

/////////////////////////// Nueva Venta ///////////////////////////
/// Instruccion: registrar_venta
#[derive(Accounts)]
#[instruction( id_cliente:u16, sku:String, descripcion:String, monto:u32)]
pub struct NuevaVenta<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = Venta::INIT_SPACE + 8,
        // El SKU entra en los seeds: un cliente no puede repetir el mismo producto
        seeds = [b"venta", id_cliente.to_le_bytes().as_ref(), sku.as_bytes(), owner.key().as_ref()],
        bump
    )]
    pub venta: Account<'info, Venta>,

    pub registro: Account<'info, Registro>, // Sin mut: solo se lee

    #[account(mut)]
    pub cliente: Account<'info, Cliente>,

    pub system_program: Program<'info, System>,
}

/////////////////////////// Modificar Venta ///////////////////////////
/// Instruccion: cambiar_monto (tambien sirve para cambiar descripcion o cualquier otro
/// campo del struct Venta)

#[derive(Accounts)]
pub struct ModificarVenta<'info> {
    pub owner: Signer<'info>,

    pub registro: Account<'info, Registro>, // Sin mut: solo se lee

    pub cliente: Account<'info, Cliente>, // Sin mut: solo se lee, para validar la cadena

    #[account(mut)]
    pub venta: Account<'info, Venta>,
}

/////////////////////////// Eliminar Venta ///////////////////////////
/// Instruccion: hacer_devolucion -> cierra la cuenta

#[derive(Accounts)]
pub struct EliminarVenta<'info> {
    #[account(mut)]
    pub owner: Signer<'info>, // mut: recibe el rent de la cuenta cerrada

    #[account(
        mut,
        close = owner
    )]
    pub venta: Account<'info, Venta>,

    #[account(mut)]
    pub registro: Account<'info, Registro>,

    #[account(mut)]
    pub cliente: Account<'info, Cliente>,
}
