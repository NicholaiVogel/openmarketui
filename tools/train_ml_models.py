#!/usr/bin/env python3
"""
Train ML models for the kalshi backtest framework.

Models:
- LSTM: learns patterns from price history sequences
- MLP: learns optimal combination of hand-crafted features

Usage:
    python scripts/train_ml_models.py --data data/trades.csv --output models/
"""

import argparse
import json
import numpy as np
import pandas as pd
from pathlib import Path

try:
    import torch
    import torch.nn as nn
    import torch.optim as optim
    from torch.utils.data import DataLoader, TensorDataset
    HAS_TORCH = True
except ImportError:
    HAS_TORCH = False
    print("warning: pytorch not installed. run: pip install torch")

def parse_args():
    parser = argparse.ArgumentParser(description="Train ML models for kalshi backtest")
    parser.add_argument("--data", type=Path, default=Path("data/trades.csv"))
    parser.add_argument("--markets", type=Path, default=Path("data/markets.csv"))
    parser.add_argument("--output", type=Path, default=Path("models"))
    parser.add_argument("--epochs", type=int, default=50)
    parser.add_argument("--batch-size", type=int, default=64)
    parser.add_argument("--seq-len", type=int, default=24)
    parser.add_argument("--train-split", type=float, default=0.8)
    return parser.parse_args()

class LSTMPredictor(nn.Module):
    def __init__(self, input_size=1, hidden_size=128, num_layers=2, dropout=0.2):
        super().__init__()
        self.lstm = nn.LSTM(
            input_size=input_size,
            hidden_size=hidden_size,
            num_layers=num_layers,
            dropout=dropout,
            batch_first=True,
        )
        self.fc = nn.Linear(hidden_size, 1)
        self.tanh = nn.Tanh()

    def forward(self, x):
        lstm_out, _ = self.lstm(x)
        last_output = lstm_out[:, -1, :]
        out = self.fc(last_output)
        return self.tanh(out)

class MLPPredictor(nn.Module):
    def __init__(self, input_size=7, hidden_sizes=[64, 32]):
        super().__init__()
        layers = []
        prev_size = input_size
        for h in hidden_sizes:
            layers.append(nn.Linear(prev_size, h))
            layers.append(nn.ReLU())
            layers.append(nn.Dropout(0.2))
            prev_size = h
        layers.append(nn.Linear(prev_size, 1))
        layers.append(nn.Tanh())
        self.net = nn.Sequential(*layers)

    def forward(self, x):
        return self.net(x)

def load_data(trades_path: Path, markets_path: Path, seq_len: int):
    print(f"loading trades from {trades_path}...")
    trades = pd.read_csv(trades_path)
    trades["timestamp"] = pd.to_datetime(trades["timestamp"])
    trades = trades.sort_values(["ticker", "timestamp"])

    print(f"loading markets from {markets_path}...")
    markets = pd.read_csv(markets_path)
    markets["close_time"] = pd.to_datetime(markets["close_time"])

    result_map = dict(zip(markets["ticker"], markets["result"]))

    sequences = []
    features = []
    labels = []

    for ticker, group in trades.groupby("ticker"):
        result = result_map.get(ticker)
        if result not in ["yes", "no"]:
            continue

        label = 1.0 if result == "yes" else -1.0

        prices = group["price"].values / 100.0
        volumes = group["volume"].values
        taker_sides = (group["taker_side"] == "yes").astype(float).values

        if len(prices) < seq_len:
            continue

        for i in range(seq_len, len(prices)):
            seq = prices[i - seq_len : i]
            log_returns = np.diff(np.log(np.clip(seq, 1e-6, 1.0)))

            if len(log_returns) == seq_len - 1:
                log_returns = np.pad(log_returns, (1, 0), mode="constant")

            sequences.append(log_returns)

            curr_price = prices[i - 1]
            momentum = prices[i - 1] - prices[i - seq_len] if len(prices) > seq_len else 0
            mean_price = np.mean(prices[i - seq_len : i])
            mean_reversion = mean_price - curr_price
            vol_sum = np.sum(volumes[i - seq_len : i])
            buy_vol = np.sum(volumes[i - seq_len : i] * taker_sides[i - seq_len : i])
            sell_vol = vol_sum - buy_vol
            order_flow = (buy_vol - sell_vol) / max(vol_sum, 1)

            feat = [
                momentum,
                mean_reversion,
                np.log1p(vol_sum),
                order_flow,
                curr_price,
                np.std(log_returns) if len(log_returns) > 1 else 0,
                len(group) / 1000.0,
            ]
            features.append(feat)
            labels.append(label)

    print(f"created {len(sequences)} training samples")
    return np.array(sequences), np.array(features), np.array(labels)

def train_lstm(sequences, labels, args):
    print("\n" + "=" * 50)
    print("Training LSTM")
    print("=" * 50)

    n = len(sequences)
    split = int(n * args.train_split)

    X_train = torch.tensor(sequences[:split], dtype=torch.float32).unsqueeze(-1)
    y_train = torch.tensor(labels[:split], dtype=torch.float32).unsqueeze(-1)
    X_test = torch.tensor(sequences[split:], dtype=torch.float32).unsqueeze(-1)
    y_test = torch.tensor(labels[split:], dtype=torch.float32).unsqueeze(-1)

    train_dataset = TensorDataset(X_train, y_train)
    train_loader = DataLoader(train_dataset, batch_size=args.batch_size, shuffle=True)

    model = LSTMPredictor(input_size=1, hidden_size=128, num_layers=2)
    criterion = nn.MSELoss()
    optimizer = optim.Adam(model.parameters(), lr=0.001)

    for epoch in range(args.epochs):
        model.train()
        total_loss = 0
        for X_batch, y_batch in train_loader:
            optimizer.zero_grad()
            output = model(X_batch)
            loss = criterion(output, y_batch)
            loss.backward()
            optimizer.step()
            total_loss += loss.item()

        if (epoch + 1) % 10 == 0:
            model.set_mode_to_inference()
            with torch.no_grad():
                train_pred = model(X_train)
                test_pred = model(X_test)
                train_acc = ((train_pred > 0) == (y_train > 0)).float().mean()
                test_acc = ((test_pred > 0) == (y_test > 0)).float().mean()
            print(f"epoch {epoch + 1}/{args.epochs}: loss={total_loss/len(train_loader):.4f}, train_acc={train_acc:.3f}, test_acc={test_acc:.3f}")

    return model

def train_mlp(features, labels, args):
    print("\n" + "=" * 50)
    print("Training MLP")
    print("=" * 50)

    features = (features - features.mean(axis=0)) / (features.std(axis=0) + 1e-8)

    n = len(features)
    split = int(n * args.train_split)

    X_train = torch.tensor(features[:split], dtype=torch.float32)
    y_train = torch.tensor(labels[:split], dtype=torch.float32).unsqueeze(-1)
    X_test = torch.tensor(features[split:], dtype=torch.float32)
    y_test = torch.tensor(labels[split:], dtype=torch.float32).unsqueeze(-1)

    train_dataset = TensorDataset(X_train, y_train)
    train_loader = DataLoader(train_dataset, batch_size=args.batch_size, shuffle=True)

    model = MLPPredictor(input_size=features.shape[1])
    criterion = nn.MSELoss()
    optimizer = optim.Adam(model.parameters(), lr=0.001)

    for epoch in range(args.epochs):
        model.train()
        total_loss = 0
        for X_batch, y_batch in train_loader:
            optimizer.zero_grad()
            output = model(X_batch)
            loss = criterion(output, y_batch)
            loss.backward()
            optimizer.step()
            total_loss += loss.item()

        if (epoch + 1) % 10 == 0:
            model.set_mode_to_inference()
            with torch.no_grad():
                train_pred = model(X_train)
                test_pred = model(X_test)
                train_acc = ((train_pred > 0) == (y_train > 0)).float().mean()
                test_acc = ((test_pred > 0) == (y_test > 0)).float().mean()
            print(f"epoch {epoch + 1}/{args.epochs}: loss={total_loss/len(train_loader):.4f}, train_acc={train_acc:.3f}, test_acc={test_acc:.3f}")

    return model

def export_onnx(model, output_path: Path, input_shape, input_name="input", output_name="output"):
    model.set_mode_to_inference()
    dummy_input = torch.randn(*input_shape)

    torch.onnx.export(
        model,
        dummy_input,
        output_path,
        input_names=[input_name],
        output_names=[output_name],
        dynamic_axes={
            input_name: {0: "batch_size"},
            output_name: {0: "batch_size"},
        },
        opset_version=14,
    )
    print(f"exported to {output_path}")

def main():
    args = parse_args()

    if not HAS_TORCH:
        print("error: pytorch required for training. install with: pip install torch")
        return 1

    if not args.data.exists():
        print(f"error: data file not found: {args.data}")
        return 1

    if not args.markets.exists():
        print(f"error: markets file not found: {args.markets}")
        return 1

    args.output.mkdir(parents=True, exist_ok=True)

    sequences, features, labels = load_data(args.data, args.markets, args.seq_len)

    if len(sequences) < 100:
        print(f"error: not enough training data ({len(sequences)} samples)")
        return 1

    lstm_model = train_lstm(sequences, labels, args)
    export_onnx(lstm_model, args.output / "lstm.onnx", (1, args.seq_len, 1))

    mlp_model = train_mlp(features, labels, args)
    export_onnx(mlp_model, args.output / "mlp.onnx", (1, features.shape[1]))

    print("\n" + "=" * 50)
    print("Training complete!")
    print(f"Models saved to: {args.output}")
    print("=" * 50)

    return 0

if __name__ == "__main__":
    exit(main())
