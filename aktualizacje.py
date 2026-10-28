import json
from vm.svl_kernel import HyperOS


if __name__ == "__main__":
    hyperos = HyperOS()
    hyperos.exec_cmd("INIT")
    hyperos.exec_cmd("RUN_HYPER")
    report = json.loads(hyperos.report())
    report["SCOPE"] = "symulacja VM; polecenia nie są wykonywane w systemie hosta"
    print(json.dumps(report, indent=4, ensure_ascii=False))