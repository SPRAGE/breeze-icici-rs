#!/usr/bin/env python3
"""Capture pinned official SDK request construction with all I/O replaced.

The SDK module is never imported: its module-level downloads, logging setup,
credentials, and Socket.IO initialization do not execute. Only the hash-checked
API class and configuration definitions are loaded. Cargo tests use the captured
fixture and require neither Python nor an installed SDK.
"""
import argparse
import ast
import enum
import hashlib
import inspect
import json
from functools import partial
from datetime import datetime
from pathlib import Path
from types import SimpleNamespace
from urllib.parse import urlsplit

REVISION = "4125106b48932ff99b45d593749dcec21c552558"
HASHES = {
    "breeze_connect.py": "430f1a97338e765141108de980ba62f0410739e09569f9849dd69e47040780a8",
    "config.py": "87d8bc428388b131bbe0f767becbf5cfe23988c922a51b03bb85f1ddb7c25f4d",
}
METHODS = dict(zip([
    "auth.customer_details", "account.demat_holdings", "account.get_funds", "account.set_funds",
    "market.historical_v1", "risk.margin_calculator", "account.get_margin", "orders.place",
    "orders.detail", "orders.list", "orders.cancel", "orders.modify", "risk.limit_price",
    "portfolio.holdings", "portfolio.positions", "market.quotes", "orders.square_off",
    "trades.list", "trades.detail", "market.option_chain", "orders.preview_equity",
    "orders.preview_fno", "market.historical_v2", "gtt.place", "gtt.list", "gtt.cancel", "gtt.modify",
], [
    "get_customer_details", "get_demat_holdings", "get_funds", "set_funds", "get_historical_data",
    "margin_calculator", "get_margin", "place_order", "get_order_detail", "get_order_list",
    "cancel_order", "modify_order", "limit_calculator", "get_portfolio_holdings",
    "get_portfolio_positions", "get_quotes", "square_off", "get_trade_list", "get_trade_detail",
    "get_option_chain_quotes", "preview_order", "preview_order", "get_historical_data_v2",
    "gtt_three_leg_place_order", "gtt_order_book", "gtt_three_leg_cancel_order", "gtt_three_leg_modify_order",
], strict=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk-dir", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    trees = {}
    for name, expected in HASHES.items():
        source = (args.sdk_dir / name).read_text().replace("\r\n", "\n").strip() + "\n"
        if hashlib.sha256(source.encode()).hexdigest() != expected:
            raise SystemExit(f"{name} is not the pinned SDK source; refusing execution")
        trees[name] = ast.parse(source, filename=name)

    config_namespace = {"enum": enum}
    definitions = [node for node in trees["config.py"].body if not isinstance(node, (ast.Import, ast.ImportFrom))]
    exec(compile(ast.Module(body=definitions, type_ignores=[]), "sdk_config", "exec"), config_namespace)
    config = SimpleNamespace(**config_namespace)
    captured = []
    preflight = []
    response = SimpleNamespace(json=lambda: {"Success": {}, "Status": 200, "Error": None})

    def capture_http(method, *, url, data="", params=None, headers):
        selected = {key.lower(): value for key, value in headers.items()
                    if key.lower() in {"content-type", "x-appkey", "apikey", "x-sessiontoken"}}
        request = {"method": method, "url": url, "path": urlsplit(url).path, "body": data, "headers": selected}
        if params is not None:
            request["query"] = params
        captured.append(request)
        return response

    def fail(*values):
        raise AssertionError(values)

    namespace = {
        "json": json, "config": config,
        "api_endpoint": config.APIEndPoint, "resp_message": config.ResponseMessage,
        "except_message": config.ExceptionMessage, "req_type": config.APIRequestType,
        "sha256": hashlib.sha256,
        "datetime": SimpleNamespace(utcnow=lambda: datetime(2025, 2, 5, 10, 0, 0)),
        "api_logger": SimpleNamespace(debug=lambda *args: None, error=lambda *args: None),
        "requests": SimpleNamespace(**{method.lower(): partial(capture_http, method)
                                      for method in ["GET", "POST", "PUT", "DELETE"]}),
    }
    api_class = next(node for node in trees["breeze_connect.py"].body if isinstance(node, ast.ClassDef) and node.name == "ApificationBreeze")
    exec(compile(ast.Module(body=[api_class], type_ignores=[]), "sdk_api_class", "exec"), namespace)
    cls = namespace["ApificationBreeze"]
    sdk = cls.__new__(cls)
    sdk.breeze = SimpleNamespace(api_key="app-key-test", secret_key="secret-key-test")
    sdk.base64_session_token = "dXNlci10ZXN0OnNlc3Npb24tdGVzdA=="
    sdk.hostname = config.API_URL
    sdk.error_exception = fail
    def order_detail(exchange, order_id):
        preflight.append({"method": "GET", "path": "/order", "body": {"exchange_code": exchange, "order_id": order_id}})
        return {"Success": [{"status": "Ordered"}]}
    sdk.get_order_detail = order_detail

    wires = json.loads((root / "tests/fixtures/wire_contracts.json").read_text())
    inputs = {}
    for key, wire in wires.items():
        values = dict(wire.get("query", [])) if wire["base"] == "rest_v2" else json.loads(wire["body"])
        if key == "auth.customer_details":
            values = {"api_session": values["SessionToken"]}
        if key == "market.historical_v1":
            values["interval"] = {"minute": "1minute", "day": "1day"}.get(values["interval"], values["interval"])
        if "exch_code" in values:
            values["exchange_code"] = values.pop("exch_code")
        if key == "risk.margin_calculator":
            values = {"lists": values["list_of_positions"], "exchange_code": values["exchange_code"]}
        if key.startswith("orders.preview"):
            values.pop("specialflag", None)  # Canonical Rust calls omit the selector.
        if key == "orders.square_off":
            values["product"] = values.pop("product_type")
            values["stoploss"] = values.pop("stoploss_price")
        inputs[key] = (METHODS[key], values)

    def variant(key, base, **changes):
        method, values = inputs[base]
        inputs[key] = (method, dict(values, **changes))

    variant("sdk.history_second", "market.historical_v2", interval="1second")
    inputs["sdk.trades_all"] = ("get_trade_list", {"exchange_code": "NSE"})
    inputs["sdk.futures_all"] = ("get_option_chain_quotes", {"stock_code": "NIFTY", "exchange_code": "NFO", "product_type": "futures"})
    for op in ["list", "cancel", "modify"]:
        variant(f"sdk.gtt_{op}_bfo", f"gtt.{op}", exchange_code="BFO")
    variant("sdk.limit_blank_product", "risk.limit_price", product_type="")
    variant("sdk.limit_source_m", "risk.limit_price", source_flag="M")
    variant("sdk.preview_stoploss_blank", "orders.preview_equity", order_type="stoploss", stoploss="")
    variant("sdk.place_vtc", "orders.place", validity="vtc")
    variant("sdk.place_routing", "orders.place", settlement_id="SETTLEMENT-TEST", order_segment_code="NSE", lots="2")
    variant("sdk.history_five", "market.historical_v2", interval="5minute")
    variant("sdk.history_thirty", "market.historical_v2", interval="30minute")
    # Independently authored inputs exercise additional valid forms. Nested
    # list semantics still come from the API reference: the SDK forwards them.
    inputs["sdk.margin_cover"] = ("margin_calculator", {"exchange_code": "NFO", "lists": [{
        "strike_price": "24000", "quantity": "75", "right": "call", "product": "options",
        "action": "buy", "price": "245.05", "expiry_date": "2025-02-27T00:00:00.000Z",
        "stock_code": "NIFTY", "cover_order_flow": "sell", "fresh_order_type": "limit",
        "cover_limit_rate": "180.00", "cover_sltp_price": "200.00", "fresh_limit_rate": "245.05", "open_quantity": "75",
    }]})
    inputs["sdk.gtt_futures"] = ("gtt_three_leg_place_order", {
        "exchange_code": "NFO", "stock_code": "NIFTY", "product": "futures", "quantity": "75",
        "expiry_date": "2025-02-27T00:00:00.000Z", "right": "others", "strike_price": "0",
        "gtt_type": "cover_oco", "fresh_order_action": "buy", "fresh_order_price": "23400", "fresh_order_type": "limit",
        "index_or_stock": "index", "trade_date": "2025-02-05T00:00:00.000Z",
        "order_details": [
            {"gtt_leg_type": "target", "action": "sell", "limit_price": "25000", "trigger_price": "24900"},
            {"gtt_leg_type": "stoploss", "action": "sell", "limit_price": "22000", "trigger_price": "22100"},
        ],
    })
    inputs["sdk.btst_placement"] = ("place_order", {"stock_code": "ITC", "exchange_code": "NSE", "product": "btst",
        "action": "buy", "order_type": "limit", "quantity": "1", "price": "420.00", "validity": "day", "user_remark": "rustsdk"})
    inputs["sdk.mtf_preview"] = ("preview_order", {"stock_code": "ITC", "exchange_code": "NSE", "product": "mtf",
        "action": "buy", "order_type": "limit", "quantity": "1", "price": "420.00"})

    cases = {}
    for key, (method, values) in inputs.items():
        captured.clear()
        preflight.clear()
        function = getattr(cls, method)
        parameters = inspect.signature(function).parameters
        supplied = {name: values.get(name, parameter.default if parameter.default is not inspect.Parameter.empty else "")
                    for name, parameter in parameters.items() if name != "self"}
        result = function(sdk, **supplied)
        if captured:
            if len(captured) != 1:
                raise AssertionError(f"{key}: expected exactly one final request")
            cases[key] = captured[0]
            if preflight:
                cases[key]["preflight"] = list(preflight)
        elif isinstance(result, dict) and result.get("Status") == 500 and result.get("Error"):
            cases[key] = {"rejected": True, "error": result["Error"]}
        else:
            raise AssertionError(f"{key}: no request and no SDK validation rejection: {result!r}")
    fixture = {"sdk_revision": REVISION, "source_sha256": HASHES, "cases": cases}
    target = root / "tests/fixtures/sdk_wire_contracts.json"
    output = json.dumps(fixture, indent=2) + "\n"
    if args.check:
        if target.read_text() != output:
            raise SystemExit("SDK request fixture is stale")
    else:
        target.write_text(output)
    print(f"captured {len(cases)} SDK cases with network I/O replaced")


if __name__ == "__main__":
    main()
