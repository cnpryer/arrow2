from typing import Tuple

import pyarrow as pa
import pyarrow.parquet
import os
from decimal import Decimal

PYARROW_PATH = "fixtures/pyarrow3"


def case_basic_nullable() -> Tuple[dict, pa.Schema, str]:
    int64 = [0, 1, None, 3, None, 5, 6, 7, None, 9]
    float64 = [0.0, 1.0, None, 3.0, None, 5.0, 6.0, 7.0, None, 9.0]
    string = ["Hello", None, "aa", "", None, "abc", None, None, "def", "aaa"]
    boolean = [True, None, False, False, None, True, None, None, True, True]
    string_large = [
        "ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD😃🌚🕳👊"
    ] * 10
    emoji = ["😃"] * 10
    decimal = [Decimal(e) if e is not None else None for e in int64]

    fields = [
        pa.field("int64", pa.int64()),
        pa.field("float64", pa.float64()),
        pa.field("string", pa.utf8()),
        pa.field("bool", pa.bool_()),
        pa.field("date", pa.timestamp("ms")),
        pa.field("uint32", pa.uint32()),
        pa.field("string_large", pa.utf8()),
        # decimal testing
        pa.field("decimal_9", pa.decimal128(9, 0)),
        pa.field("decimal_18", pa.decimal128(18, 0)),
        pa.field("decimal_26", pa.decimal128(26, 0)),
        pa.field("timestamp_us", pa.timestamp("us")),
        pa.field("timestamp_s", pa.timestamp("s")),
        pa.field("emoji", pa.utf8()),
        pa.field("timestamp_s_utc", pa.timestamp("s", "UTC")),
    ]
    schema = pa.schema(fields)

    return (
        {
            "int64": int64,
            "float64": float64,
            "string": string,
            "bool": boolean,
            "date": int64,
            "uint32": int64,
            "string_large": string_large,
            "decimal_9": decimal,
            "decimal_18": decimal,
            "decimal_26": decimal,
            "timestamp_us": int64,
            "timestamp_s": int64,
            "emoji": emoji,
            "timestamp_s_utc": int64,
        },
        schema,
        f"basic_nullable_10.parquet",
    )


def case_basic_required() -> Tuple[dict, pa.Schema, str]:
    int64 = [-256, -1, 0, 1, 2, 3, 4, 5, 6, 7]
    uint32 = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    float64 = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
    string = ["Hello", "bbb", "aa", "", "bbb", "abc", "bbb", "bbb", "def", "aaa"]
    boolean = [True, True, False, False, False, True, True, True, True, True]
    decimal = [Decimal(e) for e in int64]

    fields = [
        pa.field("int64", pa.int64(), nullable=False),
        pa.field("float64", pa.float64(), nullable=False),
        pa.field("string", pa.utf8(), nullable=False),
        pa.field("bool", pa.bool_(), nullable=False),
        pa.field(
            "date",
            pa.timestamp(
                "ms",
            ),
            nullable=False,
        ),
        pa.field("uint32", pa.uint32(), nullable=False),
        pa.field("decimal_9", pa.decimal128(9, 0), nullable=False),
        pa.field("decimal_18", pa.decimal128(18, 0), nullable=False),
        pa.field("decimal_26", pa.decimal128(26, 0), nullable=False),
    ]
    schema = pa.schema(fields)

    return (
        {
            "int64": int64,
            "float64": float64,
            "string": string,
            "bool": boolean,
            "date": int64,
            "uint32": uint32,
            "decimal_9": decimal,
            "decimal_18": decimal,
            "decimal_26": decimal,
        },
        schema,
        f"basic_required_10.parquet",
    )


def case_nested() -> Tuple[dict, pa.Schema, str]:
    items_nullable = [[0, 1], None, [2, None, 3], [4, 5, 6], [], [7, 8, 9], None, [10]]
    items_required = [[0, 1], None, [2, 0, 3], [4, 5, 6], [], [7, 8, 9], None, [10]]
    all_required = [[0, 1], [], [2, 0, 3], [4, 5, 6], [], [7, 8, 9], [], [10]]
    i16 = [[0, 1], None, [2, None, 3], [4, 5, 6], [], [7, 8, 9], None, [10]]
    boolean = [
        [False, True],
        None,
        [True, None, False],
        [True, False, True],
        [],
        [False, False, False],
        None,
        [True],
    ]
    items_nested = [
        [[0, 1]],
        None,
        [[2, None], [3]],
        [[4, 5], [6]],
        [],
        [[7], None, [9]],
        [[], [None], None],
        [[10]],
    ]
    items_required_nested = [
        [[0, 1]],
        None,
        [[2, 3], [3]],
        [[4, 5], [6]],
        [],
        [[7], None, [9]],
        None,
        [[10]],
    ]
    items_required_nested_2 = [
        [[0, 1]],
        None,
        [[2, 3], [3]],
        [[4, 5], [6]],
        [],
        [[7], [8], [9]],
        None,
        [[10]],
    ]
    string = [
        ["Hello", "bbb"],
        None,
        ["aa", None, ""],
        ["bbb", "aa", "ccc"],
        [],
        ["abc", "bbb", "bbb"],
        None,
        [""],
    ]
    fields = [
        pa.field("list_int64", pa.list_(pa.int64())),
        pa.field("list_int64_required", pa.list_(pa.field("item", pa.int64(), False))),
        pa.field(
            "list_int64_required_required",
            pa.list_(pa.field("item", pa.int64(), False)),
            False,
        ),
        pa.field("list_int16", pa.list_(pa.int16())),
        pa.field("list_bool", pa.list_(pa.bool_())),
        pa.field("list_utf8", pa.list_(pa.utf8())),
        pa.field("list_large_binary", pa.list_(pa.large_binary())),
        pa.field("list_nested_i64", pa.list_(pa.list_(pa.int64()))),
        pa.field("list_nested_inner_required_i64", pa.list_(pa.list_(pa.int64()))),
        pa.field(
            "list_nested_inner_required_required_i64", pa.list_(pa.list_(pa.int64()))
        ),
    ]
    schema = pa.schema(fields)
    return (
        {
            "list_int64": items_nullable,
            "list_int64_required": items_required,
            "list_int64_required_required": all_required,
            "list_int16": i16,
            "list_bool": boolean,
            "list_utf8": string,
            "list_large_binary": string,
            "list_nested_i64": items_nested,
            "list_nested_inner_required_i64": items_required_nested,
            "list_nested_inner_required_required_i64": items_required_nested_2,
        },
        schema,
        f"nested_nullable_10.parquet",
    )


def case_struct() -> Tuple[dict, pa.Schema, str]:
    string = ["Hello", None, "aa", "", None, "abc", None, None, "def", "aaa"]
    boolean = [True, None, False, False, None, True, None, None, True, True]
    struct_fields = [
        ("f1", pa.utf8()),
        ("f2", pa.bool_()),
    ]
    schema = pa.schema(
        [
            pa.field(
                "struct",
                pa.struct(struct_fields),
            ),
            pa.field(
                "struct_struct",
                pa.struct(
                    [
                        ("f1", pa.struct(struct_fields)),
                        ("f2", pa.bool_()),
                    ]
                ),
            ),
        ]
    )

    struct = pa.StructArray.from_arrays(
        [pa.array(string), pa.array(boolean)],
        fields=struct_fields,
    )
    return (
        {
            "struct": struct,
            "struct_struct": pa.StructArray.from_arrays(
                [struct, pa.array(boolean)],
                names=["f1", "f2"],
            ),
        },
        schema,
        f"struct_nullable_10.parquet",
    )


def case_nested_edge():
    simple = [[0, 1]]
    null = [None]
    fields = [
        pa.field("simple", pa.list_(pa.int64())),
        pa.field("null", pa.list_(pa.field("item", pa.int64(), True))),
    ]
    schema = pa.schema(fields)
    return (
        {
            "simple": simple,
            "null": null,
        },
        schema,
        f"nested_edge_nullable_10.parquet",
    )


def write_pyarrow(
    case,
    page_version: int,
    use_dictionary: bool,
    multiple_pages: bool,
    compression: str,
):
    data, schema, path = case

    base_path = f"{PYARROW_PATH}/v{page_version}"
    if use_dictionary:
        base_path = f"{base_path}/dict"

    if multiple_pages:
        base_path = f"{base_path}/multi"

    if compression:
        base_path = f"{base_path}/{compression}"

    if multiple_pages:
        data_page_size = 2 ** 10  # i.e. a small number to ensure multiple pages
    else:
        data_page_size = 2 ** 40  # i.e. a large number to ensure a single page

    t = pa.table(data, schema=schema)
    os.makedirs(base_path, exist_ok=True)
    pa.parquet.write_table(
        t,
        f"{base_path}/{path}",
        row_group_size=2 ** 40,
        use_dictionary=use_dictionary,
        compression=compression,
        write_statistics=True,
        data_page_size=data_page_size,
        data_page_version=f"{page_version}.0",
    )


for case in [case_basic_nullable, case_basic_required, case_nested, case_struct, case_nested_edge]:
    for version in [1, 2]:
        for use_dict in [True, False]:
            for compression in ["lz4", None, "snappy"]:
                write_pyarrow(case(), version, use_dict, False, compression)


def case_benches(size):
    assert size % 8 == 0
    data, schema, _ = case_basic_nullable()
    for k in data:
        data[k] = data[k][:8] * (size // 8)
    return data, schema, f"benches_{size}.parquet"


def case_benches_required(size):
    assert size % 8 == 0
    data, schema, _ = case_basic_required()
    for k in data:
        data[k] = data[k][:8] * (size // 8)
    return data, schema, f"benches_required_{size}.parquet"


# for read benchmarks
for i in range(10, 22, 2):
    # two pages (dict)
    write_pyarrow(case_benches(2 ** i), 1, True, False, None)
    # single page
    write_pyarrow(case_benches(2 ** i), 1, False, False, None)
    # single page required
    write_pyarrow(case_benches_required(2 ** i), 1, False, False, None)
    # multiple pages
    write_pyarrow(case_benches(2 ** i), 1, False, True, None)
    # multiple compressed pages
    write_pyarrow(case_benches(2 ** i), 1, False, True, "snappy")
    # single compressed page
    write_pyarrow(case_benches(2 ** i), 1, False, False, "snappy")
