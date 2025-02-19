import init, { list_methods, prepare_contract, Logic, Context, Store, init_panic_hook, DebugExternal } from "./pkg/neardebug.js";

(function(window, document) {
    async function make_context() {
        const input = document.querySelector("#input");
        const attached_deposit = document.querySelector("#attached_deposit");
        const balance = document.querySelector("#balance");
        const locked_balance = document.querySelector("#locked_balance");
        const current_account = document.querySelector("#current_account");
        const signer_account = document.querySelector("#signer_account");
        const signer_account_pk = document.querySelector("#signer_account_pk");
        const predecessor_account = document.querySelector("#predecessor_account");
        const block_height = document.querySelector("#block_height");
        const block_timestamp = document.querySelector("#block_timestamp");
        const epoch_height = document.querySelector("#epoch_height");
        const random_seed = document.querySelector("#random_seed");
        const gas = document.querySelector("#gas");
        const context = new Context()
            .input_str(input.value || input.placeholder)
            .attached_deposit(attached_deposit.value || attached_deposit.placeholder)
            .balance(balance.value || balance.placeholder)
            .locked_balance(locked_balance.value || locked_balance.placeholder)
            .current_account(current_account.value || current_account.placeholder)
            .signer_account(signer_account.value || signer_account.placeholder)
            .signer_account_pk(signer_account_pk.value || signer_account_pk.placeholder)
            .predecessor_account(predecessor_account.value || predecessor_account.placeholder)
            .block_height(block_height.value || block_height.placeholder)
            .block_timestamp(block_timestamp.value || block_timestamp.placeholder)
            .epoch_height(epoch_height.value || epoch_height.placeholder)
            .random_seed(random_seed.value || random_seed.placeholder)
            .gas(gas.value || gas.placeholder)
            ;
        return context;
    }

    async function run(method_name) {
        const contract = window.contract;
        const memory = new WebAssembly.Memory({ initial: 1024, maximum: 2048 });
        contract.memory = memory;
        const protocol_version = 72;
        const context = await make_context();
        const ext = new DebugExternal(contract.store, context, protocol_version);
        const logic = new Logic(context, memory, ext);
        contract.logic = logic;

        const import_object = { env: {} };
        import_object.internal = {
            finite_wasm_gas: (...args) => logic.finite_wasm_gas(...args),
            finite_wasm_stack: (...args) => logic.finite_wasm_stack(...args),
            finite_wasm_unstack: (...args) => logic.finite_wasm_unstack(...args),
        };
        import_object.env = {
            memory: memory,
            log_utf8: (len, ptr) => {
                try {
                    console.log(logic.get_utf8_string_free(len, ptr));
                } catch { }
                return logic.log_utf8(len, ptr);
            },

            log_utf16: (len, ptr) => {
                console.log("TODO: output log_utf16 to console");
                return logic.log_utf16(len, ptr)
            },

            read_register: (...args) => logic.read_register(...args),
            register_len: (...args) => logic.register_len(...args),
            write_register: (...args) => logic.write_register(...args),
            current_account_id: (...args) => logic.current_account_id(...args),
            signer_account_id: (...args) => logic.signer_account_id(...args),
            signer_account_pk: (...args) => logic.signer_account_pk(...args),
            predecessor_account_id: (...args) => logic.predecessor_account_id(...args),
            input: (...args) => logic.input(...args),
            block_index: (...args) => logic.block_index(...args),
            block_timestamp: (...args) => logic.block_timestamp(...args),
            epoch_height: (...args) => logic.epoch_height(...args),
            storage_usage: (...args) => logic.storage_usage(...args),
            account_balance: (...args) => logic.account_balance(...args),
            account_locked_balance: (...args) => logic.account_locked_balance(...args),
            attached_deposit: (...args) => logic.attached_deposit(...args),
            prepaid_gas: (...args) => logic.prepaid_gas(...args),
            used_gas: (...args) => logic.used_gas(...args),
            random_seed: (...args) => logic.random_seed(...args),
            sha256: (...args) => logic.sha256(...args),
            keccak256: (...args) => logic.keccak256(...args),
            keccak512: (...args) => logic.keccak512(...args),
            ed25519_verify: (...args) => logic.ed25519_verify(...args),
            ripemd160: (...args) => logic.ripemd160(...args),
            ecrecover: (...args) => logic.ecrecover(...args),
            promise_create: (...args) => logic.promise_create(...args),
            promise_then: (...args) => logic.promise_then(...args),
            promise_and: (...args) => logic.promise_and(...args),
            promise_batch_create: (...args) => logic.promise_batch_create(...args),
            promise_batch_then: (...args) => logic.promise_batch_then(...args),
            promise_batch_action_create_account: (...args) => logic.promise_batch_action_create_account(...args),
            promise_batch_action_deploy_contract: (...args) => logic.promise_batch_action_deploy_contract(...args),
            promise_batch_action_function_call: (...args) => logic.promise_batch_action_function_call(...args),
            promise_batch_action_function_call_weight: (...args) => logic.promise_batch_action_function_call_weight(...args),
            promise_batch_action_transfer: (...args) => logic.promise_batch_action_transfer(...args),
            promise_batch_action_stake: (...args) => logic.promise_batch_action_stake(...args),
            promise_batch_action_add_key_with_full_access: (...args) => logic.promise_batch_action_add_key_with_full_access(...args),
            promise_batch_action_add_key_with_function_call: (...args) => logic.promise_batch_action_add_key_with_function_call(...args),
            promise_batch_action_delete_key: (...args) => logic.promise_batch_action_delete_key(...args),
            promise_batch_action_delete_account: (...args) => logic.promise_batch_action_delete_account(...args),
            promise_yield_create: (...args) => logic.promise_yield_create(...args),
            promise_yield_resume: (...args) => logic.promise_yield_resume(...args),
            promise_results_count: () => logic.promise_results_count(...args),
            promise_result: (...args) => logic.promise_result(...args),
            promise_return: (...args) => logic.promise_return(...args),
            value_return: (...args) => logic.value_return(...args),
            panic: (...args) => logic.panic(...args),
            panic_utf8: (...args) => logic.panic_utf8(...args),
            abort: (...args) => logic.abort(...args),
            storage_write: (...args) => logic.storage_write(...args),
            storage_read: (...args) => logic.storage_read(...args),
            storage_remove: (...args) => logic.storage_remove(...args),
            storage_has_key: (...args) => logic.storage_has_key(...args),
            storage_iter_prefix: (...args) => logic.storage_iter_prefix(...args),
            storage_iter_range: (...args) => logic.storage_iter_range(...args),
            storage_iter_next: (...args) => logic.storage_iter_next(...args),
            gas: (...args) => logic.gas(...args),
            burn_gas: (...args) => logic.burn_gas(...args),
            validator_stake: (...args) => logic.validator_stake(...args),
            validator_total_stake: (...args) => logic.validator_total_stake(...args),
            alt_bn128_g1_multiexp: (...args) => logic.alt_bn128_g1_multiexp(...args),
            alt_bn128_g1_sum: (...args) => logic.alt_bn128_g1_sum(...args),
            alt_bn128_pairing_check: (...args) => logic.alt_bn128_pairing_check(...args),
            bls12381_p1_sum: (...args) => logic.bls12381_p1_sum(...args),
            bls12381_p2_sum: (...args) => logic.bls12381_p2_sum(...args),
            bls12381_g1_multiexp: (...args) => logic.bls12381_g1_multiexp(...args),
            bls12381_g2_multiexp: (...args) => logic.bls12381_g2_multiexp(...args),
            bls12381_map_fp_to_g1: (...args) => logic.bls12381_map_fp_to_g1(...args),
            bls12381_map_fp2_to_g2: (...args) => logic.bls12381_map_fp2_to_g2(...args),
            bls12381_pairing_check: (...args) => logic.bls12381_pairing_check(...args),
            bls12381_p1_decompress: (...args) => logic.bls12381_p1_decompress(...args),
            bls12381_p2_decompress: (...args) => logic.bls12381_p2_decompress(...args),
            sandbox_debug_log: () => console.warn("sandbox_debug_log is not a standard host function"),
            sleep_nanos: () => console.warn("sleep_nanos is not a standard host function"),
        };
        // NB: applying fees "before loading" does not 100% match the behaviour of nearcore --
        // nearcore would apply these fees before compiling code, but in the debugger we don't yet
        // know the method name to use at that point.
        logic.fees_before_loading_executable(method_name, BigInt(window.contract.wasm.length));
        try {
            window.contract.instance = await WebAssembly.instantiate(window.contract.module, import_object);
        } catch (e) {
            if (e.message == "HostError(GasExceeded)") {
                e.message = logic.process_gas_limit().message;
            }
            throw e;
        }
        logic.fees_after_loading_executable(BigInt(window.contract.wasm.length));
        try {
            window.contract.instance.exports[method_name]();
        } catch (e) {
            if (e.message == "HostError(GasExceeded)") {
                e.message = logic.process_gas_limit().message;
            }
            throw e;
        }
    }

    async function load(contract_data) {
        delete contract.wasm;
        delete contract.prepared_wasm;
        delete contract.instance;
        delete contract.memory;
        delete contract.logic;
        if (contract_data === undefined) {
            delete contract.module;
            return;
        }
        contract.wasm = new Uint8Array(contract_data);
        contract.prepared_wasm = prepare_contract(contract.wasm);
        contract.module = await WebAssembly.compile(contract.prepared_wasm);
    }

    async function on_contract_change(element) {
        const button = document.querySelector("#execute");
        const method_selector = document.querySelector("#methods");
        button.disabled = true;
        method_selector.innerHTML = "";
        if (element.files.length > 0) {
            const buffer = await element.files[0].arrayBuffer();
            const callable_methods = list_methods(new Uint8Array(buffer));
            for (const method of callable_methods) {
                const option = document.createElement("option");
                option.innerText = method;
                option.value = method;
                method_selector.appendChild(option);
            }
            await load(buffer);
        } else {
            await load(undefined);
        }
        button.disabled = false;
    }

    async function block_timestamp_update() {
        const update_timestamp_placeholder = () => {
            const nanos = BigInt(~~(Date.now() / 1000)) * 1000n * 1000n * 1000n;
            document.querySelector("#block_timestamp").placeholder = nanos;
        };
        update_timestamp_placeholder();
        setInterval(update_timestamp_placeholder, 1000);
    }

    async function near_input_update() {
        for (const el of document.querySelectorAll(".near_input")) {
            const input = el.querySelector("input");
            const span = el.querySelector("span");
            const update = () => {
                span.innerText = "";
                const value = (Number(input.value || input.placeholder) / 1E24);
                const formatted = value.toLocaleString(undefined, {
                    maximumFractionDigits: 3,
                    minimumFractionDigits: 1,
                    maximumSignificantDigits: 4,
                    notation: "engineering"
                });
                span.innerText = `≈ ${formatted}`;
            };
            update();
            input.addEventListener("input", update);
        }
    }

    async function gas_input_update() {
        for (const el of document.querySelectorAll(".gas_input")) {
            const input = el.querySelector("input");
            const span = el.querySelector("span");
            const update = () => {
                span.innerText = "";
                const value = Number(input.value || input.placeholder) / 1000000000000;
                span.innerText = `≈ ${value}`;
            };
            update();
            input.addEventListener("input", update);
        }
    }

    async function update_ui() {
        document.querySelector("#store_size").value = window.contract.store.size();
    }

    async function act_execute() {
        const methods = document.querySelector("#methods");
        const method = methods.selectedOptions[0].value;
        try {
            await run(method);
        } finally {
            update_ui();
        }
    }

    async function act_download_store() {
        var blob = new Blob([window.contract.store.to_json()], { type: "application/json" });
        var link = document.createElement('a');
        link.href = window.URL.createObjectURL(blob);
        link.download = `neardebug_${new Date().valueOf()}.nearstore`;
        link.click();
    }

    async function act_load_store() {
        var select = document.createElement("input");
        select.type = "file";
        select.accept = ".nearstore";
        select.onchange = async (e) => {
            const file = e.target.files[0];
            const buffer = new Uint8Array(await file.arrayBuffer());
            window.contract.store = Store.from_json(buffer);
            update_ui();
        };
        select.click();
    }

    async function on_load() {
        await init();
        init_panic_hook();
        window.contract = {
            store: new Store(),
        };
        const form = document.querySelector('#contract_form');
        form.addEventListener('submit', async (e) => {
            e.preventDefault();
            if (e.submitter.id == "execute") {
                await act_execute();
            } else if (e.submitter.id == "download_store") {
                await act_download_store();
            } else if (e.submitter.id == "load_store") {
                await act_load_store();
            }
        });

        const file_input = document.querySelector('#contract');
        file_input.addEventListener('change', (e) => {
            on_contract_change(e.target);
        });
        on_contract_change(file_input);
        block_timestamp_update();
        near_input_update();
        gas_input_update();
    }

    (window.addEventListener || window.attachEvent)('load', on_load);
})(window, document);
