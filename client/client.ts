//////////////////// Imports ////////////////////
import { PublicKey } from "@solana/web3.js";

////////////////// Constantes ////////////////////
const nombre_negocio = "Abarrotes Tsting"; // Nombre del punto de venta
const owner = pg.wallet.publicKey; // Wallet

//////////////////// Client Test Logs ////////////////////
console.log("My address:", owner.toString()); // Ver el address
const balance = await pg.connection.getBalance(owner);
console.log(`My balance: ${balance / web3.LAMPORTS_PER_SOL} SOL`); // Ver la cantidad de tokens de solana

//////////////////// FUNCIONES ////////////////////

//////////////////// OBTENER PDAs ////////////////////
/*
Un PDA representa una cuenta que es controlada por un programa (smart contract), y una de sus principales caracteristicas es no contar
con una clave privada con la cual firmar al momento de realizar alguna transaccion (transferencia, escritura o modificacion de un dato)
dentro del contrato. En su lugar, emplea direcciones generadas deterministicamente, es decir, recreables a partir de semillas.
Las semillas pueden ser varias y de diferentes tipos, puede depender desde un valor predefinido (como es usualmente el valor de la semilla 1),
hasta de direcciones secundarias (como la del caller u otra cuenta).

Es por ello que para llamar desde el front una funcion del Solana Program desplegado es necesario contar con las semillas en su orden y tipo
correspondiente. Se recomienda no usar valores sencillos (que no solo dependan de valores predefinidos), pero tampoco se encuentren
compuestas de valores redundantes (como el program id o alguna cuenta padre).

Como "owner" entra en las seeds de las tres PDAs, cada wallet opera su propio registro. En el frontend esa variable
debe salir del wallet adapter, no quedar fija.
*/
//////////////////// Helper: u16 a bytes ////////////////////
/*
En Rust las seeds numericas se escriben con &id_cliente.to_le_bytes(): little endian y 2 bytes para un u16.
Aqui hay que replicarlo byte por byte. Si usaras Buffer.from([id]) o Buffer.from(String(id)) la direccion
derivada no coincide con la del programa y Anchor falla con ConstraintSeeds.
*/
function u16ToBytes(valor) {
  const buffer = Buffer.alloc(2); // Un u16 son 2 bytes
  buffer.writeUInt16LE(valor, 0); // LE = little endian, igual que to_le_bytes() en Rust
  return buffer;
}

//////////////////// Registro ////////////////////
function pdaRegistro(nombre_negocio) {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("registro"), // Semilla 1: b"registro"
      Buffer.from(nombre_negocio), // Semilla 2: nombre del negocio -> String
      owner.toBuffer(), // Semilla 3: wallet -> Pubkey
    ],
    pg.PROGRAM_ID // Program ID: Siempre va al final
  );
}

//////////////////// Cliente ////////////////////
function pdaCliente(id_cliente) {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("cliente"), // Semilla 1: b"cliente"
      u16ToBytes(id_cliente), // Semilla 2: id del cliente -> u16 little endian
      owner.toBuffer(), // Semilla 3: wallet -> Pubkey
    ],
    pg.PROGRAM_ID // Program ID: Siempre va al final
  );
}

//////////////////// Venta ////////////////////
function pdaVenta(id_cliente, sku) {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("venta"), // Semilla 1: b"venta"
      u16ToBytes(id_cliente), // Semilla 2: id del cliente -> u16 little endian
      Buffer.from(sku), // Semilla 3: sku del producto -> String
      owner.toBuffer(), // Semilla 4: wallet -> Pubkey
    ],
    pg.PROGRAM_ID // Program ID: Siempre va al final
  );
}

//////////////////// Siguiente id disponible ////////////////////
// El id forma parte de las seeds del cliente, asi que el frontend tiene que conocerlo antes de
// construir la transaccion. El programa valida que sea exactamente id_counter + 1.
async function siguienteId(nombre_negocio) {
  const [pda_registro] = pdaRegistro(nombre_negocio);
  const registroAccount = await pg.program.account.registro.fetch(pda_registro);
  return registroAccount.idCounter + 1; // En el IDL id_counter pasa a idCounter
}

//////////////////// Crear Registro ////////////////////
// Para crear el registro solo es necesario el nombre que tendra el negocio
async function crearRegistro(nombre_negocio) {
  const [pda_registro] = pdaRegistro(nombre_negocio); // Primero se obtiene la cuenta del registro

  const txHash = await pg.program.methods // mediante la libreria pg (solana playground) se acceden a los metodos del programa
    .crearRegistro(nombre_negocio) // crear_registro -> en el IDL pasa a camelCase
    .accounts({
      // Se agregan las cuentas de las que depende (Contexto del struct NuevoRegistro)
      owner: owner,
      registro: pda_registro,
    })
    .rpc();

  console.log("txHash: ", txHash);
}

//////////////////// Crear Cliente ////////////////////
// Instruccion aparte porque "init" no puede ser condicional. Devuelve el id asignado para
// poder encadenarlo con registrarVenta.
async function crearCliente(nombre_negocio) {
  const id_cliente = await siguienteId(nombre_negocio); // El programa valida este valor

  const [pda_registro] = pdaRegistro(nombre_negocio); // se obtiene la cuenta del registro
  const [pda_cliente] = pdaCliente(id_cliente); // se determina la cuenta del cliente

  const txHash = await pg.program.methods
    .crearCliente(id_cliente) // crear_cliente
    .accounts({
      // cuentas del contexto
      owner: owner,
      registro: pda_registro,
      cliente: pda_cliente,
    })
    .rpc();

  console.log("txHash: ", txHash);
  console.log("Cliente creado con id:", id_cliente);

  return id_cliente;
}

//////////////////// Registrar Venta ////////////////////
// monto es u32 y id_cliente u16: Anchor acepta numeros de JS hasta 32 bits. A partir de u64
// hay que usar new BN(valor).
async function registrarVenta(
  nombre_negocio,
  id_cliente,
  sku,
  descripcion,
  monto
) {
  const [pda_registro] = pdaRegistro(nombre_negocio); // se obtiene la cuenta del registro
  const [pda_cliente] = pdaCliente(id_cliente); // se obtiene la cuenta del cliente
  const [pda_venta] = pdaVenta(id_cliente, sku); // se determina la cuenta de la venta

  const txHash = await pg.program.methods
    .registrarVenta(id_cliente, sku, descripcion, monto) // registrar_venta
    .accounts({
      // cuentas del contexto
      owner: owner,
      venta: pda_venta,
      registro: pda_registro,
      cliente: pda_cliente,
    })
    .rpc();

  console.log("txHash: ", txHash);
}

//////////////////// Hacer Devolucion ////////////////////
// El id y el sku son las semillas que permiten reconstruir la direccion de la venta.
// La cuenta se cierra sola gracias a 'close = owner' y el rent vuelve al owner.
async function hacerDevolucion(nombre_negocio, id_cliente, sku) {
  const [pda_registro] = pdaRegistro(nombre_negocio); // se obtiene la cuenta del registro
  const [pda_cliente] = pdaCliente(id_cliente); // se obtiene la cuenta del cliente
  const [pda_venta] = pdaVenta(id_cliente, sku); // se obtiene la cuenta de la venta a cerrar

  const txHash = await pg.program.methods
    .hacerDevolucion(id_cliente, sku) // hacer_devolucion
    .accounts({
      // cuentas del contexto
      owner: owner,
      venta: pda_venta,
      registro: pda_registro,
      cliente: pda_cliente,
    })
    .rpc();

  console.log("txHash: ", txHash);
}

//////////////////// Cambiar Monto ////////////////////
// La cuenta del cliente va aunque no se modifique: el programa la necesita para validar
// la cadena registro -> cliente -> venta.
async function cambiarMonto(nombre_negocio, id_cliente, sku, nuevo_monto) {
  const [pda_registro] = pdaRegistro(nombre_negocio); // se obtiene la cuenta del registro
  const [pda_cliente] = pdaCliente(id_cliente); // se obtiene la cuenta del cliente
  const [pda_venta] = pdaVenta(id_cliente, sku); // se obtiene la cuenta de la venta

  const txHash = await pg.program.methods
    .cambiarMonto(id_cliente, sku, nuevo_monto) // cambiar_monto
    .accounts({
      // cuentas del contexto
      owner: owner,
      registro: pda_registro,
      cliente: pda_cliente,
      venta: pda_venta,
    })
    .rpc();

  console.log("txHash: ", txHash);
}

//////////////////// Ver Clientes ////////////////////
/*
En la version anterior los clientes y sus ventas eran structs dentro de la cuenta registro, y un solo
fetch traia todo. Aqui cada cliente y cada venta son cuentas independientes: el registro ya no guarda
sus datos, guarda sus DIRECCIONES. Por eso hay que ir a buscar cada una desde el frontend, que es mas
eficiente que recorrer multiples cuentas desde el Solana Program.

Para lograr hacerlo es necesario realizar los siguientes pasos:

1. Determinar el PDA del registro
2. Obtener el vector de clientes (direcciones)
3. Por cada direccion, obtener la informacion del cliente
4. De cada cliente, obtener su vector de ventas (mas direcciones)
5. Por cada direccion de venta, obtener su informacion
6. Mostrarla con console.log
*/
async function verClientes(nombre_negocio) {
  const [pda_registro] = pdaRegistro(nombre_negocio); // se obtiene la cuenta del registro

  try {
    // Se accede a los datos de la cuenta (registro)
    const registroAccount = await pg.program.account.registro.fetch(
      pda_registro
    );

    console.log("Negocio:", registroAccount.nombreNegocio);
    console.log("Ultimo id asignado:", registroAccount.idCounter);

    // Mediante el .length se obtiene el tamaño del vector de clientes
    const numero_clientes = registroAccount.clientes.length;

    // Se verifica si hay clientes en el vector
    if (!registroAccount.clientes || numero_clientes === 0) {
      console.log("Registro vacío, aún no hay clientes");
      return;
    }

    console.log("Cantidad de clientes:", numero_clientes);

    // Se itera cada cuenta (cliente) del vector y se obtiene la informacion asociada
    for (let i = 0; i < numero_clientes; i++) {
      const clienteKey = registroAccount.clientes[i];

      const clienteAccount = await pg.program.account.cliente.fetch(clienteKey);

      console.log(
        `\nCliente #${i + 1}: \n * Id: ${
          clienteAccount.idCliente
        } \n * Compras: ${
          clienteAccount.compras
        } \n * Dirección(PDA): ${clienteKey.toBase58()}`
      );

      // Se repite el proceso con el historial: otro vector de direcciones
      for (let j = 0; j < clienteAccount.historial.length; j++) {
        const ventaKey = clienteAccount.historial[j];

        const ventaAccount = await pg.program.account.venta.fetch(ventaKey);

        console.log(
          `   Venta #${j + 1}: SKU ${ventaAccount.sku} | ${
            ventaAccount.descripcion
          } | $${ventaAccount.monto} | PDA: ${ventaKey.toBase58()}`
        );
      }
    }
  } catch (error) {
    console.error("Error viendo clientes:", error);

    // Debugging adicional
    if (error.message) {
      console.error("Mensaje de error:", error.message);
    }
    if (error.logs) {
      console.error("Logs del programa:", error.logs);
    }
  }
}

//////////////////// Alta de un cliente nuevo ////////////////////
// Son dos transacciones: crear la cuenta del cliente y luego la de su primera venta.
// Para un cliente que ya existe basta con llamar registrarVenta().
async function clienteNuevoConVenta(nombre_negocio, sku, descripcion, monto) {
  const id_cliente = await crearCliente(nombre_negocio); // Transaccion 1
  await registrarVenta(nombre_negocio, id_cliente, sku, descripcion, monto); // Transaccion 2

  console.log("Alta completa para el cliente con id:", id_cliente);
}

// crearRegistro(nombre_negocio);
// clienteNuevoConVenta(nombre_negocio, "Sabritas", "No puedes comer solo una", 30);
// registrarVenta(nombre_negocio, 2, "Coca", "Coca de sabor pineapple", 18);
// cambiarMonto(nombre_negocio, 2, "Sabritas", 32);
// hacerDevolucion(nombre_negocio, 2, "Coca");
// verClientes(nombre_negocio);

// solana confirm -v <txHash>
