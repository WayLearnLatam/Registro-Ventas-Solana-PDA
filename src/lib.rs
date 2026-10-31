use anchor_lang::prelude::*;

declare_id!("");

#[program]
pub mod registro {
    use super::*; // Importa todos los structs y enums definidos fuera del modulo

    /////////////////////////// INSTRUCCIONES ///////////////////////////
    /////////////////////////// Crear Registro ///////////////////////////
    pub fn crear_registro(context: Context<NuevoRegistro>, nombre_negocio: String) -> Result<()> {
        context.accounts.registro.set_inner(Registro {
            owner: context.accounts.owner.key(),    // caller wallet
            nombre_negocio: nombre_negocio.clone(), // .clone() porque el String se reusa en el msg!
            clientes: Vec::new(), // El negocio nace sin clientes. Guardara PDAs, no structs
            id_counter: 0,        // Contador incremental, asigna un id unico a cada cliente
        }); // Crea el struct del registro, lo serializa y lo guarda en el espacio de la cuenta

        msg!("Registro: {} creado exitosamente!", nombre_negocio); // Log de verificacion

        Ok(())
    }

    /////////////////////////// Nuevo Cliente ///////////////////////////
    // "init" no puede ser condicional: Anchor crea las cuentas antes de entrar al handler.
    // Por eso el alta del cliente es su propia instruccion, separada de registrar_venta.
    pub fn crear_cliente(context: Context<NuevoCliente>, nombre: String) -> Result<()> {
        let cliente_key = context.accounts.cliente.key(); // Se lee ANTES del &mut para no solapar prestamos

        let registro = &mut context.accounts.registro;

        // El id no es un argumento: lo asigna el programa. El contexto ya derivo la cuenta a
        // partir de este mismo valor, asi que aqui solo se confirma en el estado.
        let id_cliente = registro.id_counter + 1;
        registro.id_counter = id_cliente;
        registro.clientes.push(cliente_key); // El registro guarda la PDA del cliente, no sus datos

        context.accounts.cliente.set_inner(Cliente {
            id_cliente,
            nombre: nombre.clone(), // .clone() para no ceder la propiedad porque el String se reusa en el msg!
            compras: 0,             // Nace sin compras: registrar_venta es quien incrementa
            historial: Vec::new(),
        });

        msg!("Cliente {} creado con id: {}", nombre, id_cliente); // Log de verificacion

        Ok(())
    }

    /////////////////////////// Registrar Venta ///////////////////////////
    // id_cliente y sku no se usan en el cuerpo: viajan al contexto para derivar y validar
    // las cuentas. Ver NuevaVenta.
    pub fn registrar_venta(
        context: Context<NuevaVenta>,
        _id_cliente: u16,
        sku: String,
        descripcion: String,
        monto: u32,
    ) -> Result<()> {
        context.accounts.venta.set_inner(Venta {
            sku,
            descripcion,
            monto,
        }); // Serializa y guarda el struct en la cuenta de la venta

        let venta_key = context.accounts.venta.key(); // Se lee ANTES del &mut de cliente

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
    // y su rent vuelve al owner.
    pub fn hacer_devolucion(
        context: Context<EliminarVenta>,
        _id_cliente: u16,
        sku: String,
    ) -> Result<()> {
        let venta_key = context.accounts.venta.key(); // Se lee ANTES del &mut de cliente

        let cliente = &mut context.accounts.cliente;

        require!(cliente.historial.contains(&venta_key), Errores::NoSKU);
        // La venta existe y pertenece a este cliente (lo garantizan las seeds), pero el historial
        // es el indice de lectura: si la referencia no esta, restar compras lo desincronizaria.

        cliente.historial.retain(|&k| k != venta_key); // Quita la referencia del historial
        cliente.compras -= 1;

        msg!(
            "Venta con sku: {}, devuelta por el cliente con id: {}",
            sku,
            cliente.id_cliente
        );

        Ok(())
    }

    /////////////////////////// Cambiar Monto ///////////////////////////
    // La cuenta de la venta llega ya validada por el contexto: no hay que recorrer ningun vector
    // ni reconstruir la cadena a mano.
    pub fn cambiar_monto(
        context: Context<ModificarVenta>,
        _id_cliente: u16,
        sku: String,
        nuevo_monto: u32,
    ) -> Result<()> {
        let venta = &mut context.accounts.venta;
        let viejo_monto = venta.monto;
        venta.monto = nuevo_monto;

        msg!(
            "El monto de la venta con sku: {}, paso de: {}, a: {}",
            sku,
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
    #[msg("Error, la venta no esta en el historial del cliente")]
    NoSKU,
}

/////////////////////////// CUENTAS ///////////////////////////
/////////////////////////// Registro ///////////////////////////

#[account]
#[derive(InitSpace)]
pub struct Registro {
    pub owner: Pubkey, // pub: has_one = owner necesita leer este campo desde el contexto

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

    #[max_len(30)]
    nombre: String,

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
/*
La jerarquia del modelo vive en las seeds:

    registro  <- [b"registro", nombre_negocio, owner]
    cliente   <- [b"cliente",  registro, id_cliente]
    venta     <- [b"venta",    cliente,  sku]

Cada cuenta se deriva de su padre, asi que la pertenencia queda garantizada por construccion:
si la direccion coincide, el cliente ES de ese registro. Por eso ya no hace falta recorrer
el vector para validar; los vectores quedan como indice de lectura para el frontend.

El "has_one = owner" cierra la cadena por arriba: comprueba que registro.owner == owner.key(),
es decir, que quien firma es el dueño del registro desde el que se deriva todo lo demas.
*/

/////////////////////////// Nuevo Registro ///////////////////////////
/// Instruccion: crear_registro

#[derive(Accounts)]
#[instruction(nombre_negocio: String)]
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
pub struct NuevoCliente<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut, has_one = owner @ Errores::NoOwner)]
    pub registro: Account<'info, Registro>,

    #[account(
        init,
        payer = owner,
        space = Cliente::INIT_SPACE + 8,
        // El id sale del contador del propio registro: el programa lo asigna, nadie lo envia.
        // to_le_bytes() porque un u16 no tiene as_bytes(): ese metodo es de String/str
        seeds = [b"cliente", registro.key().as_ref(), (registro.id_counter + 1).to_le_bytes().as_ref()],
        bump
    )]
    pub cliente: Account<'info, Cliente>,

    pub system_program: Program<'info, System>,
}

/////////////////////////// Nueva Venta ///////////////////////////
/// Instruccion: registrar_venta

#[derive(Accounts)]
#[instruction(id_cliente: u16, sku: String)]
pub struct NuevaVenta<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(has_one = owner @ Errores::NoOwner)]
    pub registro: Account<'info, Registro>, // Sin mut: solo se lee para derivar al cliente

    #[account(
        mut,
        seeds = [b"cliente", registro.key().as_ref(), id_cliente.to_le_bytes().as_ref()],
        bump
    )]
    pub cliente: Account<'info, Cliente>,

    #[account(
        init,
        payer = owner,
        space = Venta::INIT_SPACE + 8,
        // El SKU entra en los seeds: un cliente no puede repetir el mismo producto
        seeds = [b"venta", cliente.key().as_ref(), sku.as_bytes()],
        bump
    )]
    pub venta: Account<'info, Venta>,

    pub system_program: Program<'info, System>,
}

/////////////////////////// Modificar Venta ///////////////////////////
/// Instruccion: cambiar_monto

#[derive(Accounts)]
#[instruction(id_cliente: u16, sku: String)]
pub struct ModificarVenta<'info> {
    pub owner: Signer<'info>,

    #[account(has_one = owner @ Errores::NoOwner)]
    pub registro: Account<'info, Registro>,

    #[account(
        seeds = [b"cliente", registro.key().as_ref(), id_cliente.to_le_bytes().as_ref()],
        bump
    )]
    pub cliente: Account<'info, Cliente>, // Sin mut: solo se lee, para derivar la venta

    #[account(
        mut,
        seeds = [b"venta", cliente.key().as_ref(), sku.as_bytes()],
        bump
    )]
    pub venta: Account<'info, Venta>,
}

/////////////////////////// Eliminar Venta ///////////////////////////
/// Instruccion: hacer_devolucion -> cierra la cuenta

#[derive(Accounts)]
#[instruction(id_cliente: u16, sku: String)]
pub struct EliminarVenta<'info> {
    #[account(mut)]
    pub owner: Signer<'info>, // mut: recibe el rent de la cuenta cerrada

    #[account(has_one = owner @ Errores::NoOwner)]
    pub registro: Account<'info, Registro>, // Sin mut: el registro no cambia en una devolucion

    #[account(
        mut,
        seeds = [b"cliente", registro.key().as_ref(), id_cliente.to_le_bytes().as_ref()],
        bump
    )]
    pub cliente: Account<'info, Cliente>,

    #[account(
        mut,
        close = owner,
        seeds = [b"venta", cliente.key().as_ref(), sku.as_bytes()],
        bump
    )]
    pub venta: Account<'info, Venta>,
}
